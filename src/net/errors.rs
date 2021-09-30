use crate::crypto::primitives::errors::ChannelError;

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
        #[snafu(display("Failed to secure the connection: {:?}", source))]
        SecureFailed { source: PlainConnectionError },
        #[snafu(display("failed to encrypt message: {}", source))]
        EncryptFailed { source: ChannelError },
        #[snafu(display("failed to decrypt message: {}", source))]
        DecryptFailed { source: ChannelError },
        #[snafu(display("failed to write: {}", source))]
        WriteFailed { source: IoError },
        #[snafu(display("failed to read: {}", source))]
        ReadFailed { source: IoError },
    }
}
