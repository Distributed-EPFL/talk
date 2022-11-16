use crate::net::plex::{Event, Message, Payload, Security};
use doomstack::{here, Doom, ResultExt, Top};
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc::{Receiver as MpscReceiver, Sender as MpscSender};

type EventInlet = MpscSender<Event>;
type EventOutlet = MpscReceiver<Event>;

type PayloadInlet = MpscSender<Payload>;
type PayloadOutlet = MpscReceiver<Payload>;

type MessageOutlet = MpscReceiver<Message>;

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
}

pub struct Plex {
    index: u32,
    run_send_inlet: EventInlet,
    receive_outlet: MessageOutlet,
}

impl Plex {
    pub(in crate::net::plex) fn new(
        index: u32,
        run_send_inlet: EventInlet,
        receive_outlet: MessageOutlet,
    ) -> Self {
        Plex {
            index,
            run_send_inlet,
            receive_outlet,
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
        let event = Event::Message {
            plex: self.index,
            message,
        };

        self.run_send_inlet
            .send(event)
            .await
            .map_err(|_| PlexError::MultiplexDropped.into_top())?;

        Ok(())
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
