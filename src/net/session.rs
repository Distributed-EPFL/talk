use crate::{
    crypto::Identity,
    net::{ConnectionSettings, SecureConnection, SecureConnectionError},
};
use doomstack::Top;
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc::Sender;

type ConnectionInlet = Sender<(Identity, SecureConnection)>;

pub struct Session {
    remote: Identity,
    connection: SecureConnection,
    return_inlet: ConnectionInlet,
}

impl Session {
    pub(in crate::net) fn new(
        remote: Identity,
        connection: SecureConnection,
        return_inlet: ConnectionInlet,
    ) -> Self {
        Session {
            remote,
            connection,
            return_inlet,
        }
    }

    pub fn configure(&mut self, settings: ConnectionSettings) {
        self.connection.configure(settings)
    }

    pub fn free_send_buffer(&mut self) {
        self.connection.free_send_buffer();
    }

    pub fn free_receive_buffer(&mut self) {
        self.connection.free_receive_buffer();
    }

    pub async fn send<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.connection.send(message).await
    }

    pub async fn send_bytes(&mut self, message: &[u8]) -> Result<(), Top<SecureConnectionError>> {
        self.connection.send_bytes(message).await
    }

    pub async fn send_plain<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.connection.send_plain(message).await
    }

    pub async fn send_plain_bytes(
        &mut self,
        message: &[u8],
    ) -> Result<(), Top<SecureConnectionError>> {
        self.connection.send_plain_bytes(message).await
    }

    pub async fn send_raw<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.connection.send_raw(message).await
    }

    pub async fn send_raw_bytes(
        &mut self,
        message: &[u8],
    ) -> Result<(), Top<SecureConnectionError>> {
        self.connection.send_raw_bytes(message).await
    }

    pub async fn receive<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: DeserializeOwned,
    {
        self.connection.receive().await
    }

    pub async fn receive_bytes(&mut self) -> Result<Vec<u8>, Top<SecureConnectionError>> {
        self.connection.receive_bytes().await
    }

    pub async fn receive_plain<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: DeserializeOwned,
    {
        self.connection.receive_plain().await
    }

    pub async fn receive_plain_bytes(&mut self) -> Result<Vec<u8>, Top<SecureConnectionError>> {
        self.connection.receive_plain_bytes().await
    }

    pub async fn receive_raw<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: DeserializeOwned,
    {
        self.connection.receive_raw().await
    }

    pub async fn receive_raw_bytes(&mut self) -> Result<Vec<u8>, Top<SecureConnectionError>> {
        self.connection.receive_raw_bytes().await
    }

    pub fn end(mut self) {
        self.connection.configure(Default::default());
        let _ = self.return_inlet.try_send((self.remote, self.connection));
    }
}
