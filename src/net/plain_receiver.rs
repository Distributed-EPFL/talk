use crate::net::{
    errors::{
        plain_connection::{DeserializeFailed, ReadFailed},
        PlainConnectionError,
    },
    Socket,
};

use serde::Deserialize;

use snafu::ResultExt;

use std::mem;

use tokio::io::{AsyncReadExt, ReadHalf};

pub struct PlainReceiver {
    read_half: ReadHalf<Box<dyn Socket>>,
    buffer: Vec<u8>,
}

impl PlainReceiver {
    pub(in crate::net) fn new(read_half: ReadHalf<Box<dyn Socket>>) -> Self {
        PlainReceiver {
            read_half,
            buffer: Vec::new(),
        }
    }

    pub(in crate::net) fn read_half(&self) -> &ReadHalf<Box<dyn Socket>> {
        &self.read_half
    }

    pub async fn receive<M>(&mut self) -> Result<M, PlainConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        let size = self.receive_size().await?;
        self.buffer.resize(size, 0);

        self.read_half
            .read_exact(&mut self.buffer[..])
            .await
            .context(ReadFailed)?;

        bincode::deserialize(&self.buffer).context(DeserializeFailed)
    }

    async fn receive_size(&mut self) -> Result<usize, PlainConnectionError> {
        let mut size = [0; mem::size_of::<u32>()];

        self.read_half
            .read_exact(&mut size[..])
            .await
            .context(ReadFailed)?;

        Ok(u32::from_le_bytes(size) as usize)
    }
}
