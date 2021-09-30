use crate::crypto::primitives::channel::{Receiver, Sender};
use crate::net::{errors::SecureConnectionError, PlainConnection};

use serde::{Deserialize, Serialize};

pub struct SecureConnection {
    channel_out: Sender,
    channel_in: Receiver,
}

impl SecureConnection {
    async fn new(mut _connection: PlainConnection) -> Self {
        todo!();
    }

    pub async fn send<M>(
        &mut self,
        _message: &M,
    ) -> Result<(), SecureConnectionError>
    where
        M: Serialize,
    {
        todo!();
    }

    pub async fn receive<M>(
        &mut self,
        _message: &M,
    ) -> Result<M, SecureConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        todo!();
    }
}
