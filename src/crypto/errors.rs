use crate::crypto::primitives::sign::SignError;

use doomstack::Top;

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum KeyCardError {
    #[snafu(display("malformed `KeyCard`: {}", source))]
    MalformedKeyCard { source: Top<SignError> },
}
