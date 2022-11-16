use crate::{
    net::{
        plex::{Cursor, Event, Header, Message, Payload, Plex, Role, Security},
        SecureConnection, SecureReceiver, SecureSender,
    },
    sync::fuse::Fuse,
};
use doomstack::{here, Doom, ResultExt, Top};
use std::collections::HashMap;
use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};

type EventInlet = MpscSender<Event>;
type EventOutlet = MpscReceiver<Event>;

type PayloadInlet = MpscSender<Payload>;
type PayloadOutlet = MpscReceiver<Payload>;

type MessageInlet = MpscSender<Message>;

// TODO: Refactor following constants into settings
const RUN_PLEX_CHANNEL_CAPACITY: usize = 128;
const RUN_ROUTE_IN_CHANNEL_CAPACITY: usize = 128;
const ROUTE_OUT_CHANNEL_CAPACITY: usize = 128;

pub(in crate::net::plex) struct Multiplex {
    cursor: Cursor,
    run_plex_inlet: EventInlet,
    _fuse: Fuse,
}

struct PlexHandle {
    receive_inlet: MessageInlet,
    _fuse: Fuse,
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
    pub fn new(role: Role, connection: SecureConnection) -> Self {
        let cursor = Cursor::new(role);

        let (run_plex_inlet, run_plex_outlet) = mpsc::channel(RUN_PLEX_CHANNEL_CAPACITY);
        let fuse = Fuse::new();

        fuse.spawn(Multiplex::run(connection, run_plex_outlet));

        Multiplex {
            cursor,
            run_plex_inlet,
            _fuse: fuse,
        }
    }

    pub async fn new_plex(&mut self) -> Plex {
        let index = self.cursor.next();
        let run_plex_inlet = self.run_plex_inlet.clone();

        Plex::new(index, run_plex_inlet).await
    }

    async fn run(
        connection: SecureConnection,
        mut run_plex_outlet: EventOutlet,
    ) -> Result<(), Top<RunError>> {
        let (sender, receiver) = connection.split();

        let (run_route_in_inlet, mut run_route_in_outlet) =
            mpsc::channel(RUN_ROUTE_IN_CHANNEL_CAPACITY);

        let (route_out_inlet, route_out_outlet) = mpsc::channel(ROUTE_OUT_CHANNEL_CAPACITY);

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
                            receive_inlet,
                            fuse,
                        } => {
                            let handle = PlexHandle {
                                receive_inlet,
                                _fuse: fuse,
                            };

                            plex_handles.insert(plex, handle);

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

                    match payload {
                        Payload::NewPlex { .. } => todo!(),
                        Payload::Message { plex, message } => {
                            if let Some(handle) = plex_handles.get_mut(&plex) {
                                let _ = handle.receive_inlet.send(message);
                            }
                        },
                        Payload::DropPlex { plex } => {
                            plex_handles.remove(&plex);
                        }
                    }
                }
            }
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
