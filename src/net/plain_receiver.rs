use crate::{
    crypto::primitives::channel::Receiver as ChannelReceiver,
    net::{
        errors::{
            plain_connection::{DeserializeFailed, ReadFailed},
            PlainConnectionError,
        },
        SecureReceiver, Socket, UnitReceiver,
    },
};

use serde::Deserialize;

use snafu::ResultExt;

use tokio::io::ReadHalf;

pub struct PlainReceiver {
    unit_receiver: UnitReceiver,
}

impl PlainReceiver {
    pub(in crate::net) fn new(read_half: ReadHalf<Box<dyn Socket>>) -> Self {
        PlainReceiver {
            unit_receiver: UnitReceiver::new(read_half),
        }
    }

    pub(in crate::net) fn read_half(&self) -> &ReadHalf<Box<dyn Socket>> {
        self.unit_receiver.read_half()
    }

    pub async fn receive<M>(&mut self) -> Result<M, PlainConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.unit_receiver.receive().await.context(ReadFailed)?;
        
        bincode::deserialize(self.unit_receiver.as_slice())
            .context(DeserializeFailed)
    }

    pub(in crate::net) fn secure(
        self,
        channel_receiver: ChannelReceiver,
    ) -> SecureReceiver {
        SecureReceiver::new(self.unit_receiver, channel_receiver)
    }
}
