#[allow(dead_code)]
#[allow(unused_imports)]
use crate::net::{
    errors::{
        plain_connection::{SerializeFailed, WriteFailed},
        PlainConnectionError, SecureConnectionError,
    },
    PlainSender, Socket,
};

use serde::Serialize;

use snafu::ResultExt;

use tokio::io::{AsyncWriteExt, WriteHalf};

pub struct SecureSender {}

impl SecureSender {
    pub(in crate::net) fn new(
        _receiver: PlainSender,
    ) -> Result<Self, SecureConnectionError> {
        todo!();
    }

    pub async fn send<M>(
        &mut self,
        _message: &M,
    ) -> Result<(), SecureConnectionError>
    where
        M: Serialize,
    {
        todo!()
    }
}
