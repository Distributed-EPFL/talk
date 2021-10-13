use crate::{net::SecureConnectionError, sync::fuse::FuseError};

use doomstack::Top;

use snafu::Snafu;

pub use receiver::ReceiverError;

pub(crate) mod receiver {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum ReceiverError {}

    pub(crate) mod listen {
        use super::*;

        #[derive(Debug, Snafu)]
        #[snafu(visibility(pub(crate)))]
        pub enum ListenError {
            #[snafu(display("`listen` interrupted: {}", source))]
            ListenInterrupted { source: Top<FuseError> },
        }
    }

    pub(crate) mod serve {
        use super::*;

        #[derive(Debug, Snafu)]
        #[snafu(visibility(pub(crate)))]
        pub enum ServeError {
            #[snafu(display("`serve` interrupted: {}", source))]
            ServeInterrupted { source: Top<FuseError> },
            #[snafu(display("connection error: {}", source))]
            ConnectionError { source: Top<SecureConnectionError> },
        }
    }

    pub(crate) mod acknowledge {
        use super::*;

        #[derive(Debug, Snafu)]
        #[snafu(visibility(pub(crate)))]
        pub enum AcknowledgeError {
            #[snafu(display("connection error: {}", source))]
            ConnectionError { source: Top<SecureConnectionError> },
        }
    }
}
