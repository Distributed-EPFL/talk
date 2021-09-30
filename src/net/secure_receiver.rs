use crate::net::{
    errors::{
        plain_connection::{DeserializeFailed, ReadFailed},
        PlainConnectionError, SecureConnectionError,
    },
    PlainReceiver, Socket,
};

use serde::Deserialize;

use snafu::ResultExt;

use std::mem;

pub struct SecureReceiver {}

impl SecureReceiver {
    pub(in crate::net) fn new(
        _receiver: PlainReceiver,
    ) -> Result<Self, SecureConnectionError> {
        todo!();
    }

    pub async fn receive<M>(&mut self) -> Result<M, SecureConnectionError>
    where
        M: for<'de> Deserialize<'de>,
    {
        todo!()
    }
}
