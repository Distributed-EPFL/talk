use crate::{
    crypto::primitives::channel::Sender as ChannelSender,
    net::{SecureConnectionError, SenderSettings, UnitSender},
};

use doomstack::{here, Doom, ResultExt, Top};

use serde::Serialize;

use tokio::time;

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

    pub async fn send<M>(
        &mut self,
        message: &M,
    ) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.channel_sender
            .encrypt_into(message, self.unit_sender.as_vec())
            .pot(SecureConnectionError::EncryptFailed, here!())?;

        let future = self.unit_sender.flush();

        let result = if let Some(send_timeout) = self.settings.send_timeout {
            time::timeout(send_timeout, future)
                .await
                .map_err(|_| SecureConnectionError::SendTimeout.into_top())
                .spot(here!())?
        } else {
            future.await
        };

        result
            .map_err(SecureConnectionError::write_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }

    pub async fn send_plain<M>(
        &mut self,
        message: &M,
    ) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.channel_sender
            .authenticate_into(message, self.unit_sender.as_vec())
            .pot(SecureConnectionError::MacComputeFailed, here!())?;

        let future = self.unit_sender.flush();

        let result = if let Some(send_timeout) = self.settings.send_timeout {
            time::timeout(send_timeout, future)
                .await
                .map_err(|_| SecureConnectionError::SendTimeout.into_top())
                .spot(here!())?
        } else {
            future.await
        };

        result
            .map_err(SecureConnectionError::write_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }
}
