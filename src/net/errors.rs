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
    pub enum SecureConnectionError {}
}
