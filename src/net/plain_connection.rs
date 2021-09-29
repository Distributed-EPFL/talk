use crate::net::{
    errors::{
        plain_connection::{
            DeserializeFailed, ReadFailed, SerializeFailed, WriteFailed,
        },
        PlainConnectionError,
    },
    Socket,
};

use serde::{Deserialize, Serialize};

use snafu::ResultExt;

use std::mem;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct PlainConnection {
    socket: Box<dyn Socket>,
    buffer: Vec<u8>,
}

impl PlainConnection {
    pub(in crate::net) fn new<S>(socket: S) -> Self
    where
        S: 'static + Socket,
    {
        PlainConnection {
            socket: Box::new(socket),
            buffer: Vec::new(),
        }
    }

    pub(in crate::net) fn from_boxed(socket: Box<dyn Socket>) -> Self {
        PlainConnection {
            socket,
            buffer: Vec::new(),
        }
    }

    pub async fn send<M>(
        &mut self,
        message: &M,
    ) -> Result<(), PlainConnectionError>
    where
        M: Serialize,
    {
        self.buffer.clear();

        bincode::serialize_into(&mut self.buffer, &message)
            .context(SerializeFailed)?;

        self.send_size(self.buffer.len()).await?;

        self.socket
            .write_all(&self.buffer[..])
            .await
            .context(WriteFailed)?;

        Ok(())
    }

    pub async fn receive<M>(&mut self) -> Result<M, PlainConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        let size = self.receive_size().await?;
        self.buffer.resize(size, 0);

        self.socket
            .read_exact(&mut self.buffer[..])
            .await
            .context(ReadFailed)?;

        bincode::deserialize(&self.buffer).context(DeserializeFailed)
    }

    async fn send_size(
        &mut self,
        size: usize,
    ) -> Result<(), PlainConnectionError> {
        let size = (size as u32).to_le_bytes();

        self.socket.write_all(&size).await.context(WriteFailed)?;

        Ok(())
    }

    async fn receive_size(&mut self) -> Result<usize, PlainConnectionError> {
        let mut size = [0; mem::size_of::<u32>()];

        self.socket
            .read_exact(&mut size[..])
            .await
            .context(ReadFailed)?;

        Ok(u32::from_le_bytes(size) as usize)
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
