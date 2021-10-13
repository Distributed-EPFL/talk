use crate::{
    errors::DynError, net::SecureConnectionError, sync::fuse::FuseError,
};

use doomstack::Top;

use snafu::Snafu;

pub use connector::ConnectorError;

pub(crate) mod connector {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum ConnectorError {
        #[snafu(display("connection failed: {}", source))]
        ConnectionFailed { source: DynError },
        #[snafu(display("connection error: {}", source))]
        ConnectionError { source: Top<SecureConnectionError> },
        #[snafu(display("context refused"))]
        ContextRefused,
    }
}

pub(crate) mod listener {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum ListenError {
        #[snafu(display("`listen` interrupted: {}", source))]
        ListenInterrupted { source: Top<FuseError> },
    }
}
