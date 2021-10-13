use crate::net::{errors::SecureConnectionError, SecureConnection};

use serde::{Deserialize, Serialize};

pub(crate) struct ConnectionPair {
    pub sender: SecureConnection,
    pub receiver: SecureConnection,
}

impl ConnectionPair {
    pub(crate) fn new(
        sender: SecureConnection,
        receiver: SecureConnection,
    ) -> Self {
        ConnectionPair { sender, receiver }
    }

    pub(crate) async fn transmit<M>(
        &mut self,
        message: &M,
    ) -> Result<M, SecureConnectionError>
    where
        M: Serialize + for<'de> Deserialize<'de>,
    {
        futures::join!(self.sender.send(message), self.receiver.receive()).1
    }
}
