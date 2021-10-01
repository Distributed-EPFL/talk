use crate::{
    crypto::primitives::channel::Receiver as ChannelReceiver,
    net::{
        errors::{
            secure_connection::{DecryptFailed, ReadFailed},
            SecureConnectionError,
        },
        Socket,
    },
};

use serde::Deserialize;

use snafu::ResultExt;

use std::mem;

use tokio::io::{AsyncReadExt, ReadHalf};

pub struct SecureReceiver {
    read_half: ReadHalf<Box<dyn Socket>>,
    unit: Vec<u8>,
    channel_receiver: ChannelReceiver,
}

impl SecureReceiver {
    pub(in crate::net) fn new(
        read_half: ReadHalf<Box<dyn Socket>>,
        unit: Vec<u8>,
        channel_receiver: ChannelReceiver,
    ) -> Self {
        Self {
            read_half,
            unit,
            channel_receiver,
        }
    }

    pub async fn receive<M>(&mut self) -> Result<M, SecureConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.receive_unit().await?;

        self.channel_receiver
            .decrypt_in_place(&mut self.unit)
            .context(DecryptFailed)
    }

    pub async fn receive_plain<M>(&mut self) -> Result<M, SecureConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.receive_unit().await?;

        self.channel_receiver
            .authenticate(&self.unit)
            .context(DecryptFailed)
    }

    async fn receive_unit(&mut self) -> Result<(), SecureConnectionError> {
        let size = self.receive_size().await?;
        self.unit.resize(size, 0);

        self.read_half
            .read_exact(&mut self.unit[..])
            .await
            .context(ReadFailed)?;

        Ok(())
    }

    async fn receive_size(&mut self) -> Result<usize, SecureConnectionError> {
        let mut size = [0; mem::size_of::<u32>()];

        self.read_half
            .read_exact(&mut size[..])
            .await
            .context(ReadFailed)?;

        Ok(u32::from_le_bytes(size) as usize)
    }
}
