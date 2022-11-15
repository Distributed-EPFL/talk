use crate::net::{SecureConnection, SecureConnectionError};

use doomstack::Top;

use serde::{de::DeserializeOwned, Serialize};

pub struct ConnectionPair {
    pub source: SecureConnection,
    pub destination: SecureConnection,
}

impl ConnectionPair {
    pub fn new(source: SecureConnection, destination: SecureConnection) -> Self {
        ConnectionPair {
            source,
            destination,
        }
    }

    pub async fn transmit<M>(&mut self, message: &M) -> Result<M, Top<SecureConnectionError>>
    where
        M: Serialize + DeserializeOwned,
    {
        futures::join!(self.source.send(message), self.destination.receive()).1
    }
}
