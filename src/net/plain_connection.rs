use crate::net::{
    errors::{PlainConnectionError, SecureConnectionError},
    PlainReceiver, PlainSender, SecureConnection, Socket,
};

use serde::{Deserialize, Serialize};

use tokio::io;

pub struct PlainConnection {
    sender: PlainSender,
    receiver: PlainReceiver,
}

impl PlainConnection {
    pub(in crate::net) fn new<S>(socket: S) -> Self
    where
        S: 'static + Socket,
    {
        PlainConnection::from_boxed(Box::new(socket))
    }

    pub(in crate::net) fn from_boxed(socket: Box<dyn Socket>) -> Self {
        let (read_half, write_half) = io::split(socket);

        let sender = PlainSender::new(write_half);
        let receiver = PlainReceiver::new(read_half);

        PlainConnection { sender, receiver }
    }

    pub fn join(
        sender: PlainSender,
        receiver: PlainReceiver,
    ) -> Result<Self, PlainConnectionError> {
        if receiver.read_half().is_pair_of(sender.send_half()) {
            Ok(Self { sender, receiver })
        } else {
            Err(PlainConnectionError::MismatchedHalves)
        }
    }

    pub async fn send<M>(
        &mut self,
        message: &M,
    ) -> Result<(), PlainConnectionError>
    where
        M: Serialize,
    {
        self.sender.send(message).await
    }

    pub async fn receive<M>(&mut self) -> Result<M, PlainConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.receiver.receive::<M>().await
    }

    pub fn split(self) -> (PlainSender, PlainReceiver) {
        (self.sender, self.receiver)
    }

    pub async fn secure(
        self,
    ) -> Result<SecureConnection, SecureConnectionError> {
        SecureConnection::new(self).await
    }
}

impl<S> From<S> for PlainConnection
where
    S: 'static + Socket,
{
    fn from(socket: S) -> Self {
        PlainConnection::new(socket)
    }
}

impl From<Box<dyn Socket>> for PlainConnection {
    fn from(socket: Box<dyn Socket>) -> Self {
        PlainConnection::from_boxed(socket)
    }
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
    async fn single_send() {
        const MESSAGE: &str = "Hello Bob, this is Alice!";

        let (bob_listener, bob_address) = new_listener().await;

        let bob_task = tokio::spawn(async move {
            let mut bob_connection: PlainConnection =
                bob_listener.accept().await.unwrap().0.into();

            let message: String = bob_connection.receive().await.unwrap();

            assert_eq!(message, MESSAGE);
        });

        let mut alice_connection: PlainConnection =
            TcpStream::connect(bob_address).await.unwrap().into();

        alice_connection.send(&String::from(MESSAGE)).await.unwrap();

        bob_task.await.unwrap();
    }

    #[tokio::test]
    async fn multiple_send() {
        let (bob_listener, bob_address) = new_listener().await;

        let bob_task = tokio::spawn(async move {
            let mut bob_connection: PlainConnection =
                bob_listener.accept().await.unwrap().0.into();

            for expected in 0..32 {
                let message: u32 = bob_connection.receive().await.unwrap();
                assert_eq!(message, expected);
            }
        });

        let mut alice_connection: PlainConnection =
            TcpStream::connect(bob_address).await.unwrap().into();

        for message in 0..32u32 {
            alice_connection.send(&message).await.unwrap();
        }

        bob_task.await.unwrap();
    }
}
