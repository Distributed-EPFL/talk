use crate::{
    crypto::Identity,
    net::{ConnectionSettings, SecureConnection, SecureConnectionError},
};

use doomstack::Top;

use serde::{Deserialize, Serialize};

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

    pub async fn send<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.connection.send(message).await
    }

    pub async fn send_plain<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.connection.send_plain(message).await
    }

    pub async fn send_raw<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.connection.send_raw(message).await
    }

    pub async fn receive<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.connection.receive().await
    }

    pub async fn receive_plain<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.connection.receive_plain().await
    }

    pub async fn receive_raw<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.connection.receive_raw().await
    }

    pub fn end(mut self) {
        self.connection.configure(Default::default());
        let _ = self.return_inlet.try_send((self.remote, self.connection));
    }
}
