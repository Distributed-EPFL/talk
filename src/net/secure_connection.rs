use crate::net::{errors::SecureConnectionError, Socket};

use serde::{Deserialize, Serialize};

pub struct SecureConnection {
    socket: Box<dyn Socket>,
    buffer: Vec<u8>,
}

impl SecureConnection {
    pub async fn send<M>(_message: &M) -> Result<(), SecureConnectionError>
    where
        M: Serialize,
    {
        todo!();
    }

    pub async fn receive<M>(_message: &M) -> Result<M, SecureConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        todo!();
    }
}
