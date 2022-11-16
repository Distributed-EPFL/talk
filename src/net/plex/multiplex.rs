use crate::{
    net::{
        plex::{
            Cursor, Event, Header, Message, MultiplexSettings, Payload, Plex, ProtoPlex, Role,
            Security,
        },
        SecureConnection, SecureReceiver, SecureSender,
    },
    sync::fuse::Fuse,
};
use doomstack::{here, Doom, ResultExt, Top};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};

type EventInlet = MpscSender<Event>;
type EventOutlet = MpscReceiver<Event>;

type PayloadInlet = MpscSender<Payload>;
type PayloadOutlet = MpscReceiver<Payload>;

type ProtoPlexInlet = MpscSender<ProtoPlex>;
type ProtoPlexOutlet = MpscReceiver<ProtoPlex>;

pub(in crate::net::plex) struct Multiplex {
    connect_multiplex: ConnectMultiplex,
    listen_multiplex: ListenMultiplex,
}

pub(in crate::net::plex) struct ConnectMultiplex {
    cursor: Cursor,
    run_plex_inlet: EventInlet,
    info: Arc<Info>,
    settings: MultiplexSettings,
    _fuse: Arc<Fuse>,
}

pub(in crate::net::plex) struct ListenMultiplex {
    accept_outlet: ProtoPlexOutlet,
    run_plex_inlet: EventInlet,
    _fuse: Arc<Fuse>,
}

struct Info {
    is_alive: AtomicBool,
    plex_count: AtomicUsize,
}

#[derive(Doom)]
pub(in crate::net::plex) enum ListenMultiplexError {
    #[doom(description("`Multiplex` dropped"))]
    MultiplexDropped,
}

#[derive(Doom)]
enum RunError {
    #[doom(description("`route_out` crashed"))]
    RouteOutError,
    #[doom(description("`route_in` crashed"))]
    RouteInError,
}

#[derive(Doom)]
enum RouteOutError {
    #[doom(description("Connection error"))]
    ConnectionError,
}

#[derive(Doom)]
enum RouteInError {
    #[doom(description("Connection error"))]
    ConnectionError,
}

impl Multiplex {
    pub fn new(role: Role, connection: SecureConnection, settings: MultiplexSettings) -> Self {
        let cursor = Cursor::new(role);

        let (run_plex_inlet, run_plex_outlet) = mpsc::channel(settings.run_plex_channel_capacity);
        let (accept_inlet, accept_outlet) = mpsc::channel(settings.accept_channel_capacity);

        let info = Info {
            is_alive: AtomicBool::new(true),
            plex_count: AtomicUsize::new(0),
        };

        let info = Arc::new(info);

        let fuse = Arc::new(Fuse::new());

        {
            let info = info.clone();
            let settings = settings.clone();

            fuse.spawn(async move {
                let _ = Multiplex::run(
                    connection,
                    run_plex_outlet,
                    accept_inlet,
                    info.as_ref(),
                    settings,
                )
                .await;

                info.is_alive.store(false, Ordering::Relaxed);
            });
        }

        let connect_multiplex = ConnectMultiplex {
            cursor,
            run_plex_inlet: run_plex_inlet.clone(),
            info: info.clone(),
            settings,
            _fuse: fuse.clone(),
        };

        let listen_multiplex = ListenMultiplex {
            accept_outlet,
            run_plex_inlet,
            _fuse: fuse,
        };

        Multiplex {
            connect_multiplex,
            listen_multiplex,
        }
    }

    pub fn split(self) -> (ConnectMultiplex, ListenMultiplex) {
        (self.connect_multiplex, self.listen_multiplex)
    }

    async fn run(
        connection: SecureConnection,
        mut run_plex_outlet: EventOutlet,
        accept_inlet: ProtoPlexInlet,
        info: &Info,
        settings: MultiplexSettings,
    ) -> Result<(), Top<RunError>> {
        let (sender, receiver) = connection.split();

        let (run_route_in_inlet, mut run_route_in_outlet) =
            mpsc::channel(settings.run_route_in_channel_capacity);

        let (route_out_inlet, route_out_outlet) =
            mpsc::channel(settings.route_out_channel_capacity);

        let fuse = Fuse::new();

        fuse.spawn(Multiplex::route_in(receiver, run_route_in_inlet));
        fuse.spawn(Multiplex::route_out(sender, route_out_outlet));

        let mut plex_handles = HashMap::new();

        loop {
            tokio::select! {
                event = run_plex_outlet.recv() => {
                    let event = if let Some(event) = event {
                        event
                    } else {
                        // `ConnectMultiplex` has dropped, shutdown
                        return Ok(());
                    };

                    let payload = match event {
                        Event::NewPlex {
                            plex,
                            handle: plex_handle
                        } => {
                            plex_handles.insert(plex, plex_handle);
                            Some(Payload::NewPlex { plex })
                        }
                        Event::Message { plex, message } => {
                            if plex_handles.contains_key(&plex) {
                                Some(Payload::Message { plex, message })
                            } else {
                                None
                            }
                        }
                        Event::DropPlex { plex } => {
                            plex_handles.remove(&plex);
                            Some(Payload::DropPlex { plex })
                        }
                        Event::Ping => Some(Payload::Ping)
                    };

                    if let Some(payload) = payload {
                        route_out_inlet
                            .send(payload)
                            .await
                            .map_err(|_| RunError::RouteOutError.into_top())
                            .spot(here!())?;
                    }
                }

                payload = run_route_in_outlet.recv() => {
                    let payload = if let Some(payload) = payload {
                        payload
                    } else {
                        return RunError::RouteInError.fail().spot(here!());
                    };

                    let response = match payload {
                        Payload::NewPlex { plex } => {
                            let (protoplex, plex_handle) = ProtoPlex::new(plex, settings.plex_settings.clone());
                            plex_handles.insert(plex, plex_handle);

                            let _ = accept_inlet.send(protoplex).await;

                            None
                        },
                        Payload::Message { plex, message } => {
                            if let Some(handle) = plex_handles.get(&plex) {
                                let _ = handle.receive_inlet.send(message);
                            }

                            None
                        },
                        Payload::DropPlex { plex } => {
                            plex_handles.remove(&plex);

                            None
                        }
                        Payload::Ping => Some(Payload::Pong),
                        Payload::Pong => None,
                    };

                    if let Some(response) = response {
                        route_out_inlet
                            .send(response)
                            .await
                            .map_err(|_| RunError::RouteOutError.into_top())
                            .spot(here!())?;
                    }
                }
            }

            info.plex_count.store(plex_handles.len(), Ordering::Relaxed);
        }
    }

    async fn route_in(
        mut receiver: SecureReceiver,
        run_route_in_inlet: PayloadInlet,
    ) -> Result<(), Top<RouteInError>> {
        loop {
            let header = receiver
                .receive::<Header>()
                .await
                .pot(RouteInError::ConnectionError, here!())?;

            let payload = match header {
                Header::NewPlex { plex } => Payload::NewPlex { plex },
                Header::Message { plex, security } => {
                    let message = match security {
                        Security::Secure => receiver.receive_bytes().await,
                        Security::Plain => receiver.receive_plain_bytes().await,
                        Security::Raw => receiver.receive_raw_bytes().await,
                    }
                    .pot(RouteInError::ConnectionError, here!())?;

                    let message = Message { security, message };

                    Payload::Message { plex, message }
                }
                Header::DropPlex { plex } => Payload::DropPlex { plex },
                Header::Ping => Payload::Ping,
                Header::Pong => Payload::Pong,
            };

            let _ = run_route_in_inlet.send(payload).await;
        }
    }

    async fn route_out(
        mut sender: SecureSender,
        mut route_out_outlet: PayloadOutlet,
    ) -> Result<(), Top<RouteOutError>> {
        loop {
            let payload = if let Some(payload) = route_out_outlet.recv().await {
                payload
            } else {
                // `ConnectMultiplex` has dropped, shutdown
                return Ok(());
            };

            sender
                .send(&payload.header())
                .await
                .pot(RouteOutError::ConnectionError, here!())?;

            match payload {
                Payload::Message { message, .. } => match message.security {
                    Security::Secure => sender.send_bytes(message.message.as_slice()).await,
                    Security::Plain => sender.send_plain_bytes(message.message.as_slice()).await,
                    Security::Raw => sender.send_raw_bytes(message.message.as_slice()).await,
                }
                .pot(RouteOutError::ConnectionError, here!())?,
                _ => (),
            }
        }
    }
}

impl ConnectMultiplex {
    pub fn is_alive(&self) -> bool {
        self.info.is_alive.load(Ordering::Relaxed)
    }

    pub fn plex_count(&self) -> usize {
        self.info.plex_count.load(Ordering::Relaxed)
    }

    pub async fn connect(&mut self) -> Plex {
        Plex::new(
            self.cursor.next(),
            self.run_plex_inlet.clone(),
            self._fuse.clone(),
            self.settings.plex_settings.clone(),
        )
        .await
    }

    pub fn ping(&self) {
        let _ = self.run_plex_inlet.try_send(Event::Ping);
    }
}

impl ListenMultiplex {
    pub async fn accept(&mut self) -> Result<Plex, Top<ListenMultiplexError>> {
        let protoplex = if let Some(protoplex) = self.accept_outlet.recv().await {
            protoplex
        } else {
            return ListenMultiplexError::MultiplexDropped.fail().spot(here!());
        };

        Ok(protoplex.into_plex(self.run_plex_inlet.clone(), self._fuse.clone()))
    }
}
