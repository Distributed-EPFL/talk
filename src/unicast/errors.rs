use crate::{
    net::errors::SecureConnectionError, sync::fuse::errors::FuseError,
};

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
            ListenInterrupted { source: FuseError },
        }
    }

    pub(crate) mod serve {
        use super::*;

        #[derive(Debug, Snafu)]
        #[snafu(visibility(pub(crate)))]
        pub enum ServeError {
            #[snafu(display("`serve` interrupted: {}", source))]
            ServeInterrupted { source: FuseError },
            #[snafu(display("connection error: {}", source))]
            ConnectionError { source: SecureConnectionError },
        }
    }

    pub(crate) mod acknowledge {
        use super::*;

        #[derive(Debug, Snafu)]
        #[snafu(visibility(pub(crate)))]
        pub enum AcknowledgeError {
            #[snafu(display("connection error: {}", source))]
            ConnectionError { source: SecureConnectionError },
        }
    }
}
