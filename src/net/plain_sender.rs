use crate::{
    crypto::primitives::channel::Sender as ChannelSender,
    net::{
        errors::{
            plain_connection::{SerializeFailed, WriteFailed},
            PlainConnectionError,
        },
        SecureSender, Socket,
    },
};

use serde::Serialize;

use snafu::ResultExt;

use tokio::io::{AsyncWriteExt, WriteHalf};

pub struct PlainSender {
    write_half: WriteHalf<Box<dyn Socket>>,
    unit: Vec<u8>,
}

impl PlainSender {
    pub(in crate::net) fn new(send_half: WriteHalf<Box<dyn Socket>>) -> Self {
        PlainSender {
            write_half: send_half,
            unit: Vec::new(),
        }
    }

    pub(in crate::net) fn send_half(&self) -> &WriteHalf<Box<dyn Socket>> {
        &self.write_half
    }

    pub async fn send<M>(
        &mut self,
        message: &M,
    ) -> Result<(), PlainConnectionError>
    where
        M: Serialize,
    {
        bincode::serialize_into(&mut self.unit, &message)
            .context(SerializeFailed)?;

        self.flush_unit().await
    }

    async fn flush_unit(&mut self) -> Result<(), PlainConnectionError> {
        self.send_size(self.unit.len()).await?;

        self.write_half
            .write_all(&self.unit)
            .await
            .context(WriteFailed)?;

        self.unit.clear();

        Ok(())
    }

    async fn send_size(
        &mut self,
        size: usize,
    ) -> Result<(), PlainConnectionError> {
        let size = (size as u32).to_le_bytes();

        self.write_half
            .write_all(&size)
            .await
            .context(WriteFailed)?;

        Ok(())
    }

    pub(in crate::net) fn secure(
        self,
        channel_sender: ChannelSender,
    ) -> SecureSender {
        SecureSender::new(self.write_half, self.unit, channel_sender)
    }
}
