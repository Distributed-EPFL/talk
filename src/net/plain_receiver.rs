use crate::{
    crypto::primitives::channel::Receiver as ChannelReceiver,
    net::{PlainConnectionError, ReceiverSettings, SecureReceiver, Socket, UnitReceiver},
    time,
};

use doomstack::{here, Doom, ResultExt, Top};

use serde::Deserialize;

use tokio::io::ReadHalf;

pub struct PlainReceiver {
    unit_receiver: UnitReceiver,
    settings: ReceiverSettings,
}

impl PlainReceiver {
    pub(in crate::net) fn new(
        read_half: ReadHalf<Box<dyn Socket>>,
        settings: ReceiverSettings,
    ) -> Self {
        PlainReceiver {
            unit_receiver: UnitReceiver::new(read_half),
            settings,
        }
    }

    pub fn configure(&mut self, settings: ReceiverSettings) {
        self.settings = settings;
    }

    pub(in crate::net) fn read_half(&self) -> &ReadHalf<Box<dyn Socket>> {
        self.unit_receiver.read_half()
    }

    pub fn free_buffer(&mut self) {
        self.unit_receiver.free_buffer();
    }

    pub async fn receive<M>(&mut self) -> Result<M, Top<PlainConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.receive_unit().await?;

        bincode::deserialize(self.unit_receiver.as_slice())
            .map_err(PlainConnectionError::deserialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }

    pub async fn receive_bytes(&mut self) -> Result<Vec<u8>, Top<PlainConnectionError>> {
        self.receive_unit().await?;

        Ok(self.unit_receiver.as_vec().clone())
    }

    async fn receive_unit(&mut self) -> Result<(), Top<PlainConnectionError>> {
        time::optional_timeout(self.settings.receive_timeout, self.unit_receiver.receive())
            .await
            .pot(PlainConnectionError::ReceiveTimeout, here!())?
            .map_err(PlainConnectionError::read_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }

    pub(in crate::net) fn secure(self, channel_receiver: ChannelReceiver) -> SecureReceiver {
        SecureReceiver::new(self.unit_receiver, channel_receiver, self.settings)
    }
}
