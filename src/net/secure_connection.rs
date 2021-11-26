use crate::{
    crypto::{
        primitives::{
            channel,
            exchange::{KeyPair, PublicKey},
            sign::Signature,
        },
        KeyCard, KeyChain, Scope, Statement, TalkHeader,
    },
    net::{ConnectionSettings, PlainConnection, SecureReceiver, SecureSender},
};

use doomstack::{here, Doom, ResultExt, Top};

use serde::{Deserialize, Serialize};

use std::io;

pub struct SecureConnection {
    sender: SecureSender,
    receiver: SecureReceiver,
    keys: Keys,
}

struct Keys {
    local: PublicKey,
    remote: PublicKey,
}

#[derive(Doom)]
pub enum SecureConnectionError {
    #[doom(description("Failed to `authenticate`"))]
    AuthenticateFailed,
    #[doom(description("Failed to decrypt message"))]
    DecryptFailed,
    #[doom(description("Failed to deserialize: {}", source))]
    #[doom(wrap(deserialize_failed))]
    DeserializeFailed { source: bincode::Error },
    #[doom(description("Failed to encrypt message"))]
    EncryptFailed,
    #[doom(description("Failed to compute message authentication code"))]
    MacComputeFailed,
    #[doom(description("Failed to verify message authentication code"))]
    MacVerifyFailed,
    #[doom(description("Failed to read: {}", source))]
    #[doom(wrap(read_failed))]
    ReadFailed { source: io::Error },
    #[doom(description("Timed out while `receive`ing"))]
    ReceiveTimeout,
    #[doom(description("Failed to `secure`"))]
    SecureFailed,
    #[doom(description("Timed out while `send`ing"))]
    SendTimeout,
    #[doom(description("Failed to serialize: {}", source))]
    #[doom(wrap(serialize_failed))]
    SerializeFailed { source: bincode::Error },
    #[doom(description("Failed to write: {}", source))]
    #[doom(wrap(write_failed))]
    WriteFailed { source: io::Error },
}

#[derive(Serialize)]
struct IdentityChallenge(PublicKey);

impl SecureConnection {
    pub(in crate::net) async fn new(
        mut connection: PlainConnection,
    ) -> Result<Self, Top<SecureConnectionError>> {
        // Run Diffie-Helman

        let keypair = KeyPair::random();
        let local_key = keypair.public();

        connection
            .send(&local_key)
            .await
            .pot(SecureConnectionError::SecureFailed, here!())?;

        let remote_key: PublicKey = connection
            .receive()
            .await
            .pot(SecureConnectionError::SecureFailed, here!())?;

        let (shared_key, role) = keypair.exchange(remote_key);

        // Create channel

        let (channel_sender, channel_receiver) = channel::channel(shared_key, role);

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

    pub fn configure(&mut self, settings: ConnectionSettings) {
        let (sender_settings, receiver_settings) = settings.split();

        self.sender.configure(sender_settings);
        self.receiver.configure(receiver_settings);
    }

    pub async fn authenticate(
        &mut self,
        keychain: &KeyChain,
    ) -> Result<KeyCard, Top<SecureConnectionError>> {
        let challenge = IdentityChallenge(self.keys.remote);
        let proof = keychain.sign(&challenge).unwrap();

        self.send(&keychain.keycard())
            .await
            .pot(SecureConnectionError::AuthenticateFailed, here!())?;

        self.send(&proof)
            .await
            .pot(SecureConnectionError::AuthenticateFailed, here!())?;

        let keycard: KeyCard = self
            .receive()
            .await
            .pot(SecureConnectionError::AuthenticateFailed, here!())?;

        let proof: Signature = self
            .receive()
            .await
            .pot(SecureConnectionError::AuthenticateFailed, here!())?;

        let challenge = IdentityChallenge(self.keys.local);

        proof
            .verify(&keycard, &challenge)
            .pot(SecureConnectionError::AuthenticateFailed, here!())?;

        Ok(keycard)
    }

    pub async fn send<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.sender.send(message).await
    }

    pub async fn send_plain<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.sender.send_plain(message).await
    }

    pub async fn send_raw<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.sender.send_raw(message).await
    }

    pub async fn receive<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.receiver.receive().await
    }

    pub async fn receive_plain<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.receiver.receive_plain().await
    }

    pub async fn receive_raw<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.receiver.receive_raw().await
    }

    pub fn split(self) -> (SecureSender, SecureReceiver) {
        (self.sender, self.receiver)
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
            let bob_connection: PlainConnection = bob_listener.accept().await.unwrap().0.into();

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
            let bob_connection: PlainConnection = bob_listener.accept().await.unwrap().0.into();

            let mut bob_connection = bob_connection.secure().await.unwrap();

            let remote_keycard = bob_connection.authenticate(&bob_keychain).await.unwrap();

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
            let bob_connection: PlainConnection = bob_listener.accept().await.unwrap().0.into();

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
    async fn single_send_plain() {
        const MESSAGE: &str = "Hello Bob, this is Alice!";

        let alice_keychain = KeyChain::random();
        let bob_keychain = KeyChain::random();

        let (bob_listener, bob_address) = new_listener().await;

        let bob_task = tokio::spawn(async move {
            let bob_connection: PlainConnection = bob_listener.accept().await.unwrap().0.into();

            let mut bob_connection = bob_connection.secure().await.unwrap();

            bob_connection.authenticate(&bob_keychain).await.unwrap();
            let message: String = bob_connection.receive_plain().await.unwrap();

            assert_eq!(message, MESSAGE);
        });

        let alice_connection: PlainConnection =
            TcpStream::connect(bob_address).await.unwrap().into();

        let mut alice_connection = alice_connection.secure().await.unwrap();

        alice_connection
            .authenticate(&alice_keychain)
            .await
            .unwrap();

        alice_connection
            .send_plain(&String::from(MESSAGE))
            .await
            .unwrap();

        bob_task.await.unwrap();
    }

    #[tokio::test]
    async fn multiple_send() {
        let alice_keychain = KeyChain::random();
        let bob_keychain = KeyChain::random();

        let (bob_listener, bob_address) = new_listener().await;

        let bob_task = tokio::spawn(async move {
            let bob_connection: PlainConnection = bob_listener.accept().await.unwrap().0.into();

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

    #[tokio::test]
    async fn multiple_send_plain() {
        let alice_keychain = KeyChain::random();
        let bob_keychain = KeyChain::random();

        let (bob_listener, bob_address) = new_listener().await;

        let bob_task = tokio::spawn(async move {
            let bob_connection: PlainConnection = bob_listener.accept().await.unwrap().0.into();

            let mut bob_connection = bob_connection.secure().await.unwrap();

            bob_connection.authenticate(&bob_keychain).await.unwrap();

            for expected in 0..32 {
                let message: u32 = bob_connection.receive_plain().await.unwrap();
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
            alice_connection.send_plain(&message).await.unwrap();
        }

        bob_task.await.unwrap();
    }

    #[tokio::test]
    async fn mixed_send() {
        let alice_keychain = KeyChain::random();
        let bob_keychain = KeyChain::random();

        let (bob_listener, bob_address) = new_listener().await;

        let bob_task = tokio::spawn(async move {
            let bob_connection: PlainConnection = bob_listener.accept().await.unwrap().0.into();

            let mut bob_connection = bob_connection.secure().await.unwrap();

            bob_connection.authenticate(&bob_keychain).await.unwrap();

            for expected in 0..32 {
                let message: u32 = if expected % 2 == 0 {
                    bob_connection.receive().await.unwrap()
                } else {
                    bob_connection.receive_plain().await.unwrap()
                };

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
            if message % 2 == 0 {
                alice_connection.send(&message).await.unwrap();
            } else {
                alice_connection.send_plain(&message).await.unwrap();
            }
        }

        bob_task.await.unwrap();
    }
}
