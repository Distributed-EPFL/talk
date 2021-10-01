use crate::{
    crypto::primitives::channel::Sender as ChannelSender,
    net::{
        errors::{
            secure_connection::{EncryptFailed, WriteFailed},
            SecureConnectionError,
        },
        UnitSender,
    },
};

use serde::Serialize;

use snafu::ResultExt;

pub struct SecureSender {
    unit_sender: UnitSender,
    channel_sender: ChannelSender,
}

impl SecureSender {
    pub(in crate::net) fn new(
        unit_sender: UnitSender,
        channel_sender: ChannelSender,
    ) -> Self {
        Self {
            unit_sender,
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
            .encrypt_into(message, self.unit_sender.as_vec())
            .context(EncryptFailed)?;

        self.unit_sender.flush().await.context(WriteFailed)
    }

    pub async fn send_plain<M>(
        &mut self,
        message: &M,
    ) -> Result<(), SecureConnectionError>
    where
        M: Serialize,
    {
        self.channel_sender
            .authenticate_into(message, self.unit_sender.as_vec())
            .context(EncryptFailed)?;

        self.unit_sender.flush().await.context(WriteFailed)
    }
}
