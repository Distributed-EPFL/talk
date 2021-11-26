use crate::{
    crypto::primitives::channel::Sender as ChannelSender,
    net::{SecureConnectionError, SenderSettings, UnitSender},
    time,
};

use doomstack::{here, Doom, ResultExt, Top};

use serde::Serialize;

pub struct SecureSender {
    unit_sender: UnitSender,
    channel_sender: ChannelSender,
    settings: SenderSettings,
}

impl SecureSender {
    pub(in crate::net) fn new(
        unit_sender: UnitSender,
        channel_sender: ChannelSender,
        settings: SenderSettings,
    ) -> Self {
        Self {
            unit_sender,
            channel_sender,
            settings,
        }
    }

    pub fn configure(&mut self, settings: SenderSettings) {
        self.settings = settings;
    }

    pub async fn send<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.channel_sender
            .encrypt_into(message, self.unit_sender.as_vec())
            .pot(SecureConnectionError::EncryptFailed, here!())?;

        time::optional_timeout(self.settings.send_timeout, self.unit_sender.flush())
            .await
            .pot(SecureConnectionError::SendTimeout, here!())?
            .map_err(SecureConnectionError::write_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }

    pub async fn send_plain<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.channel_sender
            .authenticate_into(message, self.unit_sender.as_vec())
            .pot(SecureConnectionError::MacComputeFailed, here!())?;

        time::optional_timeout(self.settings.send_timeout, self.unit_sender.flush())
            .await
            .pot(SecureConnectionError::SendTimeout, here!())?
            .map_err(SecureConnectionError::write_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }

    pub async fn send_raw<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        bincode::serialize_into(self.unit_sender.as_vec(), &message)
            .map_err(SecureConnectionError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        time::optional_timeout(self.settings.send_timeout, self.unit_sender.flush())
            .await
            .pot(SecureConnectionError::SendTimeout, here!())?
            .map_err(SecureConnectionError::write_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }
}
