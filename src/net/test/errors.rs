use crate::{
    crypto::primitives::sign::PublicKey, net::errors::SecureConnectionError,
    sync::fuse::FuseError,
};

use doomstack::Top;

use snafu::Snafu;

use std::io::Error as IoError;

pub(crate) use test_connector::ConnectorError;

pub(crate) mod test_connector {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum ConnectorError {
        #[snafu(display("address unknown"))]
        AddressUnknown,
        #[snafu(display("connection failed: {}", source))]
        ConnectionFailed { source: IoError },
        #[snafu(display("`secure` failed: {}", source))]
        SecureFailed { source: SecureConnectionError },
        #[snafu(display("`authenticate` failed: {}", source))]
        AuthenticateFailed { source: SecureConnectionError },
        #[snafu(display("unexpected remote: {:?}", remote))]
        UnexpectedRemote { remote: PublicKey },
    }
}

pub(crate) mod test_listener {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum ListenError {
        #[snafu(display("`listen` interrupted: {}", source))]
        ListenInterrupted { source: Top<FuseError> },
    }
}
