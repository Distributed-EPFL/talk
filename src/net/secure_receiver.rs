use crate::{
    crypto::primitives::channel::Receiver as ChannelReceiver,
    net::{
        errors::{
            secure_connection::{DecryptFailed, ReadFailed},
            SecureConnectionError,
        },
        UnitReceiver,
    },
};

use serde::Deserialize;

use snafu::ResultExt;

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

    pub async fn receive<M>(&mut self) -> Result<M, SecureConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.unit_receiver.receive().await.context(ReadFailed)?;

        self.channel_receiver
            .decrypt_in_place(self.unit_receiver.as_vec())
            .context(DecryptFailed)
    }

    pub async fn receive_plain<M>(&mut self) -> Result<M, SecureConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.unit_receiver.receive().await.context(ReadFailed)?;

        self.channel_receiver
            .authenticate(self.unit_receiver.as_vec())
            .context(DecryptFailed)
    }
}
