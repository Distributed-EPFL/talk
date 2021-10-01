use crate::{
    crypto::primitives::channel::Sender as ChannelSender,
    net::{
        errors::{
            secure_connection::{EncryptFailed, WriteFailed},
            SecureConnectionError,
        },
        Socket,
    },
};

use serde::Serialize;

use snafu::ResultExt;

use tokio::io::{AsyncWriteExt, WriteHalf};

pub struct SecureSender {
    write_half: WriteHalf<Box<dyn Socket>>,
    unit: Vec<u8>,
    channel_sender: ChannelSender,
}

impl SecureSender {
    pub(in crate::net) fn new(
        write_half: WriteHalf<Box<dyn Socket>>,
        unit: Vec<u8>,
        channel_sender: ChannelSender,
    ) -> Self {
        Self {
            write_half,
            unit,
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
        self.channel_sender
            .encrypt_into(message, &mut self.unit)
            .context(EncryptFailed)?;

        self.flush_unit().await
    }

    pub async fn send_plain<M>(
        &mut self,
        message: &M,
    ) -> Result<(), SecureConnectionError>
    where
        M: Serialize,
    {
        self.channel_sender
            .authenticate_into(message, &mut self.unit)
            .context(EncryptFailed)?;

        self.flush_unit().await
    }

    async fn flush_unit(&mut self) -> Result<(), SecureConnectionError> {
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
    ) -> Result<(), SecureConnectionError> {
        let size = (size as u32).to_le_bytes();

        self.write_half
            .write_all(&size)
            .await
            .context(WriteFailed)?;

        Ok(())
    }
}
