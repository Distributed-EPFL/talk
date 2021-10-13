use crate::{
    crypto::primitives::channel::Receiver as ChannelReceiver,
    net::{SecureConnectionError, UnitReceiver},
};

use doomstack::{here, Doom, ResultExt, Top};

use serde::Deserialize;

pub struct SecureReceiver {
    unit_receiver: UnitReceiver,
    channel_receiver: ChannelReceiver,
}

impl SecureReceiver {
    pub(in crate::net) fn new(
        unit_receiver: UnitReceiver,
        channel_receiver: ChannelReceiver,
    ) -> Self {
        Self {
            unit_receiver,
            channel_receiver,
        }
    }

    pub async fn receive<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.unit_receiver
            .receive()
            .await
            .map_err(SecureConnectionError::read_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        self.channel_receiver
            .decrypt_in_place(self.unit_receiver.as_vec())
            .pot(SecureConnectionError::DecryptFailed, here!())
    }

    pub async fn receive_plain<M>(
        &mut self,
    ) -> Result<M, Top<SecureConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.unit_receiver
            .receive()
            .await
            .map_err(SecureConnectionError::read_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        self.channel_receiver
            .authenticate(self.unit_receiver.as_vec())
            .pot(SecureConnectionError::MacVerifyFailed, here!())
    }
}
