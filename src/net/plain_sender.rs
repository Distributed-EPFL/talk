use crate::{
    crypto::primitives::channel::Sender as ChannelSender,
    net::{
        PlainConnectionError, SecureSender, SenderSettings, Socket, UnitSender,
    },
};

use doomstack::{here, Doom, ResultExt, Top};

use serde::Serialize;

use tokio::io::WriteHalf;
use tokio::time;

pub struct PlainSender {
    unit_sender: UnitSender,
    settings: SenderSettings,
}

impl PlainSender {
    pub(in crate::net) fn new(
        write_half: WriteHalf<Box<dyn Socket>>,
        settings: SenderSettings,
    ) -> Self {
        PlainSender {
            unit_sender: UnitSender::new(write_half),
            settings,
        }
    }

    pub fn configure(&mut self, settings: SenderSettings) {
        self.settings = settings;
    }

    pub(in crate::net) fn write_half(&self) -> &WriteHalf<Box<dyn Socket>> {
        self.unit_sender.write_half()
    }

    pub async fn send<M>(
        &mut self,
        message: &M,
    ) -> Result<(), Top<PlainConnectionError>>
    where
        M: Serialize,
    {
        bincode::serialize_into(self.unit_sender.as_vec(), &message)
            .map_err(PlainConnectionError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        let future = self.unit_sender.flush();

        let result = if let Some(send_timeout) = self.settings.send_timeout {
            time::timeout(send_timeout, future)
                .await
                .map_err(|_| PlainConnectionError::SendTimeout.into_top())
                .spot(here!())?
        } else {
            future.await
        };

        result
            .map_err(PlainConnectionError::write_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }

    pub(in crate::net) fn secure(
        self,
        channel_sender: ChannelSender,
    ) -> SecureSender {
        SecureSender::new(self.unit_sender, channel_sender, self.settings)
    }
}
