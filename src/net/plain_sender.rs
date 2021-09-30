use crate::net::{
    errors::{
        plain_connection::{SerializeFailed, WriteFailed},
        PlainConnectionError,
    },
    Socket,
};

use serde::Serialize;

use snafu::ResultExt;

use tokio::io::{AsyncWriteExt, WriteHalf};

pub struct PlainSender {
    send_half: WriteHalf<Box<dyn Socket>>,
    buffer: Vec<u8>,
}

impl PlainSender {
    pub(in crate::net) fn new(send_half: WriteHalf<Box<dyn Socket>>) -> Self {
        PlainSender {
            send_half,
            buffer: Vec::new(),
        }
    }

    pub(in crate::net) fn send_half(&self) -> &WriteHalf<Box<dyn Socket>> {
        &self.send_half
    }

    pub async fn send<M>(
        &mut self,
        message: &M,
    ) -> Result<(), PlainConnectionError>
    where
        M: Serialize,
    {
        self.buffer.clear();

        bincode::serialize_into(&mut self.buffer, &message)
            .context(SerializeFailed)?;

        self.send_size(self.buffer.len()).await?;

        self.send_half
            .write_all(&self.buffer[..])
            .await
            .context(WriteFailed)?;

        Ok(())
    }

    async fn send_size(
        &mut self,
        size: usize,
    ) -> Result<(), PlainConnectionError> {
        let size = (size as u32).to_le_bytes();

        self.send_half.write_all(&size).await.context(WriteFailed)?;

        Ok(())
    }
}
