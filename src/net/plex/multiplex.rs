use crate::{
    net::{
        plex::{Event, Header, Payload, Role, Security},
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

// TODO: Refactor following constants into settings
const RUN_SEND_CHANNEL_CAPACITY: usize = 128;
const RUN_ROUTE_IN_CHANNEL_CAPACITY: usize = 128;
const ROUTE_OUT_CHANNEL_CAPACITY: usize = 128;

pub(in crate::net::plex) struct Multiplex {
    cursor: Cursor,
    run_send_inlet: EventInlet,
    _fuse: Fuse,
}

struct Cursor {
    role: Role,
    cursor: u32,
}

#[derive(Doom)]
pub enum RouteOutError {
    #[doom(description("Connection error"))]
    ConnectionError,
}

#[derive(Doom)]
pub enum RouteInError {
    #[doom(description("Connection error"))]
    ConnectionError,
}

impl Multiplex {
    pub fn new(role: Role, connection: SecureConnection) -> Self {
        let cursor = Cursor::new(role);

        let (run_send_inlet, run_send_outlet) = mpsc::channel(RUN_SEND_CHANNEL_CAPACITY);
        let fuse = Fuse::new();

        fuse.spawn(Multiplex::run(connection, run_send_outlet));

        Multiplex {
            cursor,
            run_send_inlet,
            _fuse: fuse,
        }
    }

    async fn run(connection: SecureConnection, mut run_send_outlet: EventOutlet) {
        let (sender, receiver) = connection.split();

        let (route_out_inlet, route_out_outlet) = mpsc::channel(ROUTE_OUT_CHANNEL_CAPACITY);

        let (run_route_in_inlet, run_route_in_outlet) =
            mpsc::channel(RUN_ROUTE_IN_CHANNEL_CAPACITY);

        let fuse = Fuse::new();

        fuse.spawn(Multiplex::route_in(receiver, run_route_in_inlet));
        fuse.spawn(Multiplex::route_out(sender, route_out_outlet));

        let mut receive_inlets = HashMap::new();

        loop {
            let event = if let Some(event) = run_send_outlet.recv().await {
                event
            } else {
                // `ConnectMultiplex` has dropped, shutdown
                return;
            };

            match event {
                Event::NewPlex {
                    plex,
                    message_inlet,
                } => {
                    if receive_inlets.insert(plex, message_inlet).is_some() {
                        return;
                    }
                }
                Event::Message {
                    plex,
                    security,
                    message,
                } => todo!(),
                Event::DropPlex { plex } => todo!(),
            }
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
                Payload::Message {
                    message, security, ..
                } => match security {
                    Security::Secure => {
                        sender
                            .send_bytes(message.as_slice())
                            .await
                            .pot(RouteOutError::ConnectionError, here!())?;
                    }
                    Security::Plain => {
                        sender
                            .send_plain_bytes(message.as_slice())
                            .await
                            .pot(RouteOutError::ConnectionError, here!())?;
                    }
                    Security::Raw => {
                        sender
                            .send_raw_bytes(message.as_slice())
                            .await
                            .pot(RouteOutError::ConnectionError, here!())?;
                    }
                },
                _ => (),
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
                Header::NewPlex { plex } => unimplemented!(),
                Header::Message { plex, security } => {
                    let message = match security {
                        Security::Secure => receiver
                            .receive_bytes()
                            .await
                            .pot(RouteInError::ConnectionError, here!())?,
                        Security::Plain => receiver
                            .receive_plain_bytes()
                            .await
                            .pot(RouteInError::ConnectionError, here!())?,
                        Security::Raw => receiver
                            .receive_raw_bytes()
                            .await
                            .pot(RouteInError::ConnectionError, here!())?,
                    };

                    Payload::Message {
                        plex,
                        security,
                        message,
                    }
                }
                Header::DropPlex { plex } => Payload::DropPlex { plex },
            };

            let _ = run_route_in_inlet.send(payload).await;
        }
    }
}

impl Cursor {
    fn new(role: Role) -> Self {
        match role {
            Role::Connector => Cursor {
                role: Role::Connector,
                cursor: 0,
            },
            Role::Listener => Cursor {
                role: Role::Listener,
                cursor: u32::MAX,
            },
        }
    }

    fn next(&mut self) -> u32 {
        let next = self.cursor;

        match self.role {
            Role::Connector => {
                self.cursor += 1;
            }
            Role::Listener => {
                self.cursor -= 1;
            }
        }

        next
    }
}