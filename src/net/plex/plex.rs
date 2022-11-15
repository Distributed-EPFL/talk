use crate::net::plex::{Event, Payload, Security};
use doomstack::{here, Doom, ResultExt, Top};
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc::{Receiver as MpscReceiver, Sender as MpscSender};

type EventOutlet = MpscReceiver<Event>;

type PayloadInlet = MpscSender<Payload>;
type PayloadOutlet = MpscReceiver<Payload>;

type EventInlet = MpscSender<Event>;
type MessageOutlet = MpscReceiver<(Security, Vec<u8>)>;

#[derive(Doom)]
pub enum PlexError {
    #[doom(description("Failed to serialize: {:?}", source))]
    #[doom(wrap(serialize_failed))]
    SerializeFailed { source: bincode::Error },
    #[doom(description("Failed to deserialize: {:?}", source))]
    #[doom(wrap(deserialize_failed))]
    DeserializeFailed { source: bincode::Error },
    #[doom(description("Multiplex dropped"))]
    MultiplexDropped,
    #[doom(description("Unexpected security level"))]
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

        self.send_security(message, Security::Secure).await
    }

    pub async fn send_bytes(&mut self, message: &[u8]) -> Result<(), Top<PlexError>> {
        self.send_security(message.to_vec(), Security::Secure).await
    }

    pub async fn send_plain<M>(&mut self, message: &M) -> Result<(), Top<PlexError>>
    where
        M: Serialize,
    {
        let message = bincode::serialize(&message)
            .map_err(PlexError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        self.send_security(message, Security::Plain).await
    }

    pub async fn send_plain_bytes(&mut self, message: &[u8]) -> Result<(), Top<PlexError>> {
        self.send_security(message.to_vec(), Security::Plain).await
    }

    pub async fn send_raw<M>(&mut self, message: &M) -> Result<(), Top<PlexError>>
    where
        M: Serialize,
    {
        let message = bincode::serialize(&message)
            .map_err(PlexError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        self.send_security(message, Security::Raw).await
    }

    pub async fn send_raw_bytes(&mut self, message: &[u8]) -> Result<(), Top<PlexError>> {
        self.send_security(message.to_vec(), Security::Raw).await
    }

    async fn send_security(
        &mut self,
        message: Vec<u8>,
        security: Security,
    ) -> Result<(), Top<PlexError>> {
        let event = Event::Message {
            plex: self.index,
            security,
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
        self.receive_bytes_security(Security::Secure).await
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
        self.receive_bytes_security(Security::Plain).await
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
        self.receive_bytes_security(Security::Raw).await
    }

    async fn receive_bytes_security(
        &mut self,
        expected_security: Security,
    ) -> Result<Vec<u8>, Top<PlexError>> {
        let (security, message) = self
            .receive_outlet
            .recv()
            .await
            .ok_or_else(|| PlexError::MultiplexDropped.into_top())?;

        if expected_security == security {
            Ok(message)
        } else {
            PlexError::UnexpectedSecurity.fail().spot(here!())
        }
    }
}
