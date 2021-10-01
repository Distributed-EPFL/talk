use crate::{
    crypto::primitives::channel::Sender as ChannelSender,
    net::{
        errors::{
            plain_connection::{SerializeFailed, WriteFailed},
            PlainConnectionError,
        },
        SecureSender, Socket, UnitSender,
    },
};

use serde::Serialize;

use snafu::ResultExt;

use tokio::io::WriteHalf;

pub struct PlainSender {
    unit_sender: UnitSender,
}

impl PlainSender {
    pub(in crate::net) fn new(write_half: WriteHalf<Box<dyn Socket>>) -> Self {
        PlainSender {
            unit_sender: UnitSender::new(write_half),
        }
    }

    pub(in crate::net) fn write_half(&self) -> &WriteHalf<Box<dyn Socket>> {
        self.unit_sender.write_half()
    }

    pub async fn send<M>(
        &mut self,
        message: &M,
    ) -> Result<(), PlainConnectionError>
    where
        M: Serialize,
    {
        bincode::serialize_into(self.unit_sender.as_vec(), &message)
            .context(SerializeFailed)?;

        self.unit_sender.flush().await.context(WriteFailed)
    }

    pub(in crate::net) fn secure(
        self,
        channel_sender: ChannelSender,
    ) -> SecureSender {
        SecureSender::new(self.unit_sender, channel_sender)
    }
}
