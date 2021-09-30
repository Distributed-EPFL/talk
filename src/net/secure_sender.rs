use crate::{
    crypto::primitives::channel::Sender as ChannelSender,
    net::{
        errors::{
            plain_connection::SerializeFailed,
            secure_connection::{EncryptFailed, WriteFailed},
            PlainConnectionError, SecureConnectionError,
        },
        PlainSender, Socket,
    },
};

use serde::Serialize;

use snafu::ResultExt;

use tokio::io::{AsyncWriteExt, WriteHalf};

pub struct SecureSender {
    write_half: WriteHalf<Box<dyn Socket>>,
    buffer: Vec<u8>,
    channel_sender: ChannelSender,
}

impl SecureSender {
    pub(in crate::net) fn new(
        write_half: WriteHalf<Box<dyn Socket>>,
        buffer: Vec<u8>,
        channel_sender: ChannelSender,
    ) -> Self {
        Self {
            write_half,
            buffer,
            channel_sender,
        }
    }

    pub async fn send<M>(
        &mut self,
        message: &M,
    ) -> Result<(), SecureConnectionError>
    where
        M: Serialize,
    {
        self.buffer.clear();

        self.channel_sender
            .encrypt_into(message, &mut self.buffer)
            .context(EncryptFailed)?;

        self.send_size(self.buffer.len()).await?;

        self.write_half
            .write_all(&self.buffer[..])
            .await
            .context(WriteFailed)?;

        Ok(())
    }

    async fn send_size(
        &mut self,
        size: usize,
    ) -> Result<(), SecureConnectionError> {
        let size = (size as u32).to_le_bytes();

        self.write_half
            .write_all(&size)
            .await
            .context(WriteFailed)?;

        Ok(())
    }
}
