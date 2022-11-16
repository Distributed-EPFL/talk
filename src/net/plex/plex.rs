use crate::{
    net::plex::{Event, Message, Payload, Security},
    sync::fuse::{Fuse, Relay},
};
use doomstack::{here, Doom, ResultExt, Top};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};

type EventInlet = MpscSender<Event>;
type EventOutlet = MpscReceiver<Event>;

type PayloadInlet = MpscSender<Payload>;
type PayloadOutlet = MpscReceiver<Payload>;

type MessageInlet = MpscSender<Message>;
type MessageOutlet = MpscReceiver<Message>;

// TODO: Refactor following constants into settings
const RECEIVE_CHANNEL_CAPACITY: usize = 32;

#[derive(Doom)]
pub enum PlexError {
    #[doom(description("Failed to serialize: {:?}", source))]
    #[doom(wrap(serialize_failed))]
    SerializeFailed { source: bincode::Error },
    #[doom(description("Failed to deserialize: {:?}", source))]
    #[doom(wrap(deserialize_failed))]
    DeserializeFailed { source: bincode::Error },
    #[doom(description("`Multiplex` dropped"))]
    MultiplexDropped,
    #[doom(description("Unexpected `Security` level"))]
    UnexpectedSecurity,
    #[doom(description("`Plex` closed"))]
    PlexClosed,
}

pub struct Plex {
    index: u32,

    run_plex_inlet: EventInlet,
    receive_outlet: MessageOutlet,

    plex_relay: Relay,
    _run_fuse: Arc<Fuse>,
}

pub(in crate::net::plex) struct ProtoPlex {
    index: u32,
    receive_outlet: MessageOutlet,
    relay: Relay,
}

pub(in crate::net::plex) struct PlexHandle {
    pub receive_inlet: MessageInlet,
    pub _fuse: Fuse,
}

impl Plex {
    pub(in crate::net::plex) async fn new(
        index: u32,
        run_plex_inlet: EventInlet,
        run_fuse: Arc<Fuse>,
    ) -> Self {
        let (receive_inlet, receive_outlet) = mpsc::channel(RECEIVE_CHANNEL_CAPACITY);

        let fuse = Fuse::new();
        let relay = fuse.relay();

        let handle = PlexHandle {
            receive_inlet,
            _fuse: fuse,
        };

        let _ = run_plex_inlet
            .send(Event::NewPlex {
                plex: index,
                handle,
            })
            .await;

        Plex {
            index,
            run_plex_inlet,
            receive_outlet,
            plex_relay: relay,
            _run_fuse: run_fuse,
        }
    }

    pub async fn send<M>(&mut self, message: &M) -> Result<(), Top<PlexError>>
    where
        M: Serialize,
    {
        let message = bincode::serialize(&message)
            .map_err(PlexError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        self.send_message(Message {
            security: Security::Secure,
            message,
        })
        .await
    }

    pub async fn send_bytes(&mut self, message: &[u8]) -> Result<(), Top<PlexError>> {
        self.send_message(Message {
            security: Security::Secure,
            message: message.to_vec(),
        })
        .await
    }

    pub async fn send_plain<M>(&mut self, message: &M) -> Result<(), Top<PlexError>>
    where
        M: Serialize,
    {
        let message = bincode::serialize(&message)
            .map_err(PlexError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        self.send_message(Message {
            security: Security::Plain,
            message,
        })
        .await
    }

    pub async fn send_plain_bytes(&mut self, message: &[u8]) -> Result<(), Top<PlexError>> {
        self.send_message(Message {
            security: Security::Plain,
            message: message.to_vec(),
        })
        .await
    }

    pub async fn send_raw<M>(&mut self, message: &M) -> Result<(), Top<PlexError>>
    where
        M: Serialize,
    {
        let message = bincode::serialize(&message)
            .map_err(PlexError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        self.send_message(Message {
            security: Security::Raw,
            message,
        })
        .await
    }

    pub async fn send_raw_bytes(&mut self, message: &[u8]) -> Result<(), Top<PlexError>> {
        self.send_message(Message {
            security: Security::Raw,
            message: message.to_vec(),
        })
        .await
    }

    async fn send_message(&mut self, message: Message) -> Result<(), Top<PlexError>> {
        if self.plex_relay.is_on() {
            let event = Event::Message {
                plex: self.index,
                message,
            };

            self.run_plex_inlet
                .send(event)
                .await
                .map_err(|_| PlexError::MultiplexDropped.into_top())
        } else {
            PlexError::PlexClosed.fail().spot(here!())
        }
    }

    pub async fn receive<M>(&mut self) -> Result<M, Top<PlexError>>
    where
        M: DeserializeOwned,
    {
        let message = self.receive_bytes().await?;

        bincode::deserialize(&message)
            .map_err(PlexError::deserialize_failed)
            .map_err(PlexError::into_top)
            .spot(here!())
    }

    pub async fn receive_bytes(&mut self) -> Result<Vec<u8>, Top<PlexError>> {
        let message = self.receive_message(Security::Secure).await?;
        Ok(message.message)
    }

    pub async fn receive_plain<M>(&mut self) -> Result<M, Top<PlexError>>
    where
        M: DeserializeOwned,
    {
        let message = self.receive_plain_bytes().await?;

        bincode::deserialize(&message)
            .map_err(PlexError::deserialize_failed)
            .map_err(PlexError::into_top)
            .spot(here!())
    }

    pub async fn receive_plain_bytes(&mut self) -> Result<Vec<u8>, Top<PlexError>> {
        let message = self.receive_message(Security::Plain).await?;
        Ok(message.message)
    }

    pub async fn receive_raw<M>(&mut self) -> Result<M, Top<PlexError>>
    where
        M: DeserializeOwned,
    {
        let message = self.receive_raw_bytes().await?;

        bincode::deserialize(&message)
            .map_err(PlexError::deserialize_failed)
            .map_err(PlexError::into_top)
            .spot(here!())
    }

    pub async fn receive_raw_bytes(&mut self) -> Result<Vec<u8>, Top<PlexError>> {
        let message = self.receive_message(Security::Raw).await?;
        Ok(message.message)
    }

    async fn receive_message(&mut self, security: Security) -> Result<Message, Top<PlexError>> {
        let message = self
            .receive_outlet
            .recv()
            .await
            .ok_or_else(|| PlexError::MultiplexDropped.into_top())?;

        if message.security == security {
            Ok(message)
        } else {
            PlexError::UnexpectedSecurity.fail().spot(here!())
        }
    }
}

impl ProtoPlex {
    pub fn new(index: u32) -> (ProtoPlex, PlexHandle) {
        let (receive_inlet, receive_outlet) = mpsc::channel(RECEIVE_CHANNEL_CAPACITY);

        let fuse = Fuse::new();
        let relay = fuse.relay();

        let protoplex = ProtoPlex {
            index,
            receive_outlet,
            relay,
        };

        let plex_handle = PlexHandle {
            receive_inlet,
            _fuse: fuse,
        };

        (protoplex, plex_handle)
    }

    pub fn into_plex(self, run_plex_inlet: EventInlet, run_fuse: Arc<Fuse>) -> Plex {
        Plex {
            index: self.index,
            run_plex_inlet,
            receive_outlet: self.receive_outlet,
            plex_relay: self.relay,
            _run_fuse: run_fuse,
        }
    }
}

impl Drop for Plex {
    fn drop(&mut self) {
        let run_plex_inlet = self.run_plex_inlet.clone();
        let plex = self.index;

        tokio::spawn(async move {
            let _ = run_plex_inlet.send(Event::DropPlex { plex }).await;
        });
    }
}
