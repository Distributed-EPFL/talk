use crate::crypto::primitives::errors::sign::SignError;

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum KeyCardError {
    #[snafu(display("malformed `KeyCard`: {}", source))]
    MalformedKeyCard { source: SignError },
}
