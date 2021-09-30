use crate::{
    crypto::{
        primitives::{
            channel,
            exchange::{KeyPair, PublicKey},
            sign::Signature,
        },
        KeyCard, KeyChain, Scope, Statement, TalkHeader,
    },
    net::{
        errors::{
            secure_connection::{AuthenticationFailed, SecureFailed},
            SecureConnectionError,
        },
        PlainConnection, SecureReceiver, SecureSender,
    },
};

use serde::{Deserialize, Serialize};

use snafu::ResultExt;

pub struct SecureConnection {
    sender: SecureSender,
    receiver: SecureReceiver,
    keys: Keys,
}

struct Keys {
    local: PublicKey,
    remote: PublicKey,
}

#[derive(Serialize)]
struct IdentityChallenge(PublicKey);

impl SecureConnection {
    pub(in crate::net) async fn new(
        mut connection: PlainConnection,
    ) -> Result<Self, SecureConnectionError> {
        // Run Diffie-Helman

        let keypair = KeyPair::random();
        let local_key = keypair.public();

        connection.send(&local_key).await.context(SecureFailed)?;

        let remote_key: PublicKey =
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
            keys: Keys {
                local: local_key,
                remote: remote_key,
            },
        })
    }

    pub async fn authenticate(
        &mut self,
        keychain: &KeyChain,
    ) -> Result<KeyCard, SecureConnectionError> {
        let challenge = IdentityChallenge(self.keys.remote);
        let proof = keychain.sign(&challenge).unwrap();

        self.send(&keychain.keycard()).await?;
        self.send(&proof).await?;

        let keycard: KeyCard = self.receive().await?;
        let proof: Signature = self.receive().await?;

        let challenge = IdentityChallenge(self.keys.local);

        proof
            .verify(&keycard, &challenge)
            .context(AuthenticationFailed)?;

        Ok(keycard)
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

impl Statement for IdentityChallenge {
    const SCOPE: Scope = Scope::talk();
    type Header = TalkHeader;
    const HEADER: TalkHeader = TalkHeader::SecureConnectionIdentityChallenge;
}
