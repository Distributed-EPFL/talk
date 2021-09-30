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

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::SocketAddr;

    use tokio::net::{TcpListener, TcpStream};

    async fn new_listener() -> (TcpListener, SocketAddr) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();

        (listener, address)
    }

    #[tokio::test]
    async fn secure() {
        let (bob_listener, bob_address) = new_listener().await;

        let bob_task = tokio::spawn(async move {
            let bob_connection: PlainConnection =
                bob_listener.accept().await.unwrap().0.into();

            bob_connection.secure().await.unwrap();
        });

        let alice_connection: PlainConnection =
            TcpStream::connect(bob_address).await.unwrap().into();

        alice_connection.secure().await.unwrap();

        bob_task.await.unwrap();
    }

    #[tokio::test]
    async fn authenticate() {
        let alice_keychain = KeyChain::random();
        let bob_keychain = KeyChain::random();

        let alice_keycard = alice_keychain.keycard();
        let bob_keycard = bob_keychain.keycard();

        let (bob_listener, bob_address) = new_listener().await;

        let bob_task = tokio::spawn(async move {
            let bob_connection: PlainConnection =
                bob_listener.accept().await.unwrap().0.into();

            let mut bob_connection = bob_connection.secure().await.unwrap();

            let remote_keycard =
                bob_connection.authenticate(&bob_keychain).await.unwrap();

            assert_eq!(remote_keycard, alice_keycard);
        });

        let alice_connection: PlainConnection =
            TcpStream::connect(bob_address).await.unwrap().into();

        let mut alice_connection = alice_connection.secure().await.unwrap();

        let remote_keycard = alice_connection
            .authenticate(&alice_keychain)
            .await
            .unwrap();

        assert_eq!(remote_keycard, bob_keycard);

        bob_task.await.unwrap();
    }

    #[tokio::test]
    async fn single_send() {
        const MESSAGE: &str = "Hello Bob, this is Alice!";

        let alice_keychain = KeyChain::random();
        let bob_keychain = KeyChain::random();

        let (bob_listener, bob_address) = new_listener().await;

        let bob_task = tokio::spawn(async move {
            let bob_connection: PlainConnection =
                bob_listener.accept().await.unwrap().0.into();

            let mut bob_connection = bob_connection.secure().await.unwrap();

            bob_connection.authenticate(&bob_keychain).await.unwrap();
            let message: String = bob_connection.receive().await.unwrap();

            assert_eq!(message, MESSAGE);
        });

        let alice_connection: PlainConnection =
            TcpStream::connect(bob_address).await.unwrap().into();

        let mut alice_connection = alice_connection.secure().await.unwrap();

        alice_connection
            .authenticate(&alice_keychain)
            .await
            .unwrap();

        alice_connection.send(&String::from(MESSAGE)).await.unwrap();

        bob_task.await.unwrap();
    }

    #[tokio::test]
    async fn multiple_send() {
        let alice_keychain = KeyChain::random();
        let bob_keychain = KeyChain::random();

        let (bob_listener, bob_address) = new_listener().await;

        let bob_task = tokio::spawn(async move {
            let bob_connection: PlainConnection =
                bob_listener.accept().await.unwrap().0.into();

            let mut bob_connection = bob_connection.secure().await.unwrap();

            bob_connection.authenticate(&bob_keychain).await.unwrap();

            for expected in 0..32 {
                let message: u32 = bob_connection.receive().await.unwrap();
                assert_eq!(message, expected);
            }
        });

        let alice_connection: PlainConnection =
            TcpStream::connect(bob_address).await.unwrap().into();

        let mut alice_connection = alice_connection.secure().await.unwrap();

        alice_connection
            .authenticate(&alice_keychain)
            .await
            .unwrap();

        for message in 0..32u32 {
            alice_connection.send(&message).await.unwrap();
        }

        bob_task.await.unwrap();
    }
}
