use crate::crypto::primitives::{channel::ChannelError, sign::SignError};

use doomstack::Top;

use snafu::Snafu;

use std::io::Error as IoError;

type BincodeError = Box<bincode::ErrorKind>;

pub use plain_connection::PlainConnectionError;
pub use secure_connection::SecureConnectionError;

pub(crate) mod plain_connection {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum PlainConnectionError {
        #[snafu(display("mismatched halves"))]
        MismatchedHalves,
        #[snafu(display("failed to serialize data: {}", source))]
        SerializeFailed { source: BincodeError },
        #[snafu(display("failed to deserialize data: {}", source))]
        DeserializeFailed { source: BincodeError },
        #[snafu(display("failed to write: {}", source))]
        WriteFailed { source: IoError },
        #[snafu(display("failed to read: {}", source))]
        ReadFailed { source: IoError },
    }
}

pub(crate) mod secure_connection {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum SecureConnectionError {
        #[snafu(display("failed to secure the connection: {:?}", source))]
        SecureFailed { source: PlainConnectionError },
        #[snafu(display(
            "failed to authenticate the connection: {:?}",
            source
        ))]
        AuthenticationFailed { source: Top<SignError> },
        #[snafu(display("failed to encrypt message: {}", source))]
        EncryptFailed { source: Top<ChannelError> },
        #[snafu(display("failed to decrypt message: {}", source))]
        DecryptFailed { source: Top<ChannelError> },
        #[snafu(display("failed to write: {}", source))]
        WriteFailed { source: IoError },
        #[snafu(display("failed to read: {}", source))]
        ReadFailed { source: IoError },
    }
}
