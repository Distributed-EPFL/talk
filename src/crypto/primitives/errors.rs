use ed25519_dalek::SignatureError as EdSignatureError;

use snafu::Snafu;

type BincodeError = Box<bincode::ErrorKind>;

pub use hash::HashError;
pub use sign::SignError;

pub(crate) mod hash {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum HashError {
        #[snafu(display("failed to serialize data: {}", source))]
        SerializeFailed { source: BincodeError },
    }
}

pub(crate) mod sign {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum SignError {
        #[snafu(display("failed to serialize data: {}", source))]
        SerializeFailed { source: BincodeError },
        #[snafu(display("failed to verify signature: {}", source))]
        VerifyFailed { source: EdSignatureError },
    }
}
