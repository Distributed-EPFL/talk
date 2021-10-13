use blst::BLST_ERROR;

use ed25519_dalek::SignatureError as EdSignatureError;

use snafu::Snafu;

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

type BincodeError = Box<bincode::ErrorKind>;

pub use multi::MultiError;
pub use sign::SignError;

pub(crate) mod sign {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum SignError {
        #[snafu(display("malformed public key: {}", source))]
        MalformedPublicKey { source: EdSignatureError },
        #[snafu(display("failed to serialize data: {}", source))]
        SerializeFailed { source: BincodeError },
        #[snafu(display("failed to verify signature: {}", source))]
        VerifyFailed { source: EdSignatureError },
    }
}

pub(crate) mod multi {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum MultiError {
        #[snafu(display("malformed public key: {}", source))]
        MalformedPublicKey { source: BlstError },
        #[snafu(display("failed to serialize data: {}", source))]
        SerializeFailed { source: BincodeError },
        #[snafu(display("failed to verify signature: {}", source))]
        VerifyFailed { source: BlstError },
        #[snafu(display("malformed signature: {}", source))]
        MalformedSignature { source: BlstError },
        #[snafu(display("failed to aggregate signatures: {}", source))]
        AggregateFailed { source: BlstError },
    }

    #[derive(Debug)]
    pub struct BlstError(BLST_ERROR);

    impl From<BLST_ERROR> for BlstError {
        fn from(error: BLST_ERROR) -> Self {
            BlstError(error)
        }
    }

    impl Error for BlstError {}

    impl Display for BlstError {
        fn fmt(&self, f: &mut Formatter) -> FmtResult {
            let s = match self.0 {
                BLST_ERROR::BLST_POINT_NOT_IN_GROUP => "point not in group",
                BLST_ERROR::BLST_AGGR_TYPE_MISMATCH => {
                    "signature type mismatched"
                }
                BLST_ERROR::BLST_PK_IS_INFINITY => "public key is infinity",
                BLST_ERROR::BLST_SUCCESS => "no error",
                BLST_ERROR::BLST_BAD_ENCODING => "bad encoding",
                BLST_ERROR::BLST_POINT_NOT_ON_CURVE => "point not on curve",
                BLST_ERROR::BLST_VERIFY_FAIL => "bad signature",
                BLST_ERROR::BLST_BAD_SCALAR => "bad scalar",
            };

            write!(f, "{}", s)
        }
    }
}
