use crate::{
    crypto::primitives::{
        channel,
        exchange::{KeyPair, PublicKey as ExchangePublicKey},
        sign::PublicKey as SignPublicKey,
    },
    net::{
        errors::{secure_connection::SecureFailed, SecureConnectionError},
        PlainConnection, SecureReceiver, SecureSender,
    },
};

use serde::{Deserialize, Serialize};

use snafu::ResultExt;

pub struct SecureConnection {
    sender: SecureSender,
    receiver: SecureReceiver,
    remote_key: ExchangePublicKey,
}

impl SecureConnection {
    pub(in crate::net) async fn new(
        mut connection: PlainConnection,
    ) -> Result<Self, SecureConnectionError> {
        // Run Diffie-Helman

        let keypair = KeyPair::random();

        connection
            .send(&keypair.public())
            .await
            .context(SecureFailed)?;

        let remote_key: ExchangePublicKey =
            connection.receive().await.context(SecureFailed)?;

        let (shared_key, role) = keypair.exchange(remote_key);

        // Create channel

        let (channel_sender, channel_receiver) =
            channel::channel(shared_key, role);

        // Create Secure Sender and Receiver

        let (plain_sender, plain_receiver) = connection.split();

        Ok(Self {
            sender: plain_sender.secure(channel_sender),
            receiver: plain_receiver.secure(channel_receiver),
            remote_key: remote_key,
        })
    }

    pub async fn authenticate(
        &mut self,
    ) -> Result<SignPublicKey, SecureConnectionError> {
        todo!()
    }

    pub async fn send<M>(
        &mut self,
        message: &M,
    ) -> Result<(), SecureConnectionError>
    where
        M: Serialize,
    {
        self.sender.send(message).await
    }

    pub async fn receive<M>(&mut self) -> Result<M, SecureConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.receiver.receive().await
    }
}
