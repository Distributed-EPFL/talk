use crate::{
    crypto::primitives::channel::Receiver as ChannelReceiver,
    net::{ReceiverSettings, SecureConnectionError, UnitReceiver},
    time,
};
use doomstack::{here, Doom, ResultExt, Top};
use serde::de::DeserializeOwned;

pub struct SecureReceiver {
    unit_receiver: UnitReceiver,
    channel_receiver: ChannelReceiver,
    settings: ReceiverSettings,
}

impl SecureReceiver {
    pub(in crate::net) fn new(
        unit_receiver: UnitReceiver,
        channel_receiver: ChannelReceiver,
        settings: ReceiverSettings,
    ) -> Self {
        Self {
            unit_receiver,
            channel_receiver,
            settings,
        }
    }

    pub fn configure(&mut self, settings: ReceiverSettings) {
        self.settings = settings;
    }

    pub fn free_buffer(&mut self) {
        self.unit_receiver.free_buffer();
    }

    pub async fn receive<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: DeserializeOwned,
    {
        self.receive_unit().await?;

        self.channel_receiver
            .decrypt_in_place(self.unit_receiver.as_vec())
            .pot(SecureConnectionError::DecryptFailed, here!())
    }

    pub async fn receive_bytes(&mut self) -> Result<Vec<u8>, Top<SecureConnectionError>> {
        self.receive_unit().await?;

        self.channel_receiver
            .decrypt_bytes(self.unit_receiver.as_vec())
            .pot(SecureConnectionError::DecryptFailed, here!())
    }

    pub async fn receive_plain<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: DeserializeOwned,
    {
        self.receive_unit().await?;

        self.channel_receiver
            .authenticate(self.unit_receiver.as_vec())
            .pot(SecureConnectionError::MacVerifyFailed, here!())
    }

    pub async fn receive_plain_bytes(&mut self) -> Result<Vec<u8>, Top<SecureConnectionError>> {
        self.receive_unit().await?;

        let message = self
            .channel_receiver
            .authenticate_bytes(self.unit_receiver.as_vec())
            .pot(SecureConnectionError::MacVerifyFailed, here!())?;

        Ok(message.to_vec())
    }

    pub async fn receive_raw<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: DeserializeOwned,
    {
        self.receive_unit().await?;

        bincode::deserialize(self.unit_receiver.as_slice())
            .map_err(SecureConnectionError::deserialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }

    pub async fn receive_raw_bytes(&mut self) -> Result<Vec<u8>, Top<SecureConnectionError>> {
        self.receive_unit().await?;

        Ok(self.unit_receiver.as_vec().clone())
    }

    async fn receive_unit(&mut self) -> Result<(), Top<SecureConnectionError>> {
        time::optional_timeout(self.settings.receive_timeout, self.unit_receiver.receive())
            .await
            .pot(SecureConnectionError::ReceiveTimeout, here!())?
            .map_err(SecureConnectionError::read_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }
}
