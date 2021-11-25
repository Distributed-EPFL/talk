use crate::{
    crypto::Identity,
    net::{ConnectionSettings, SecureConnection, SecureConnectionError},
};

use doomstack::Top;

use serde::{Deserialize, Serialize};

use tokio::sync::mpsc::Sender;

type ReturnInlet = Sender<(crate::crypto::Identity, SecureConnection)>;

pub struct Session {
    remote: Identity,
    connection: Option<SecureConnection>,
    return_inlet: ReturnInlet,
}

impl Session {
    pub(in crate::net) fn new(
        remote: Identity,
        connection: SecureConnection,
        return_inlet: ReturnInlet,
    ) -> Self {
        Session {
            remote,
            connection: Some(connection),
            return_inlet,
        }
    }

    pub fn configure(&mut self, settings: ConnectionSettings) {
        self.connection.as_mut().unwrap().configure(settings)
    }

    pub async fn send<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.connection.as_mut().unwrap().send(message).await
    }

    pub async fn send_plain<M>(&mut self, message: &M) -> Result<(), Top<SecureConnectionError>>
    where
        M: Serialize,
    {
        self.connection.as_mut().unwrap().send_plain(message).await
    }

    pub async fn receive<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.connection.as_mut().unwrap().receive().await
    }

    pub async fn receive_plain<M>(&mut self) -> Result<M, Top<SecureConnectionError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        self.connection.as_mut().unwrap().receive_plain().await
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self
            .return_inlet
            .try_send((self.remote, self.connection.take().unwrap()));
    }
}
