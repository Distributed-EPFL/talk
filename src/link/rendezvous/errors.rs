use crate::{
    link::rendezvous::ShardId, net::errors::PlainConnectionError,
    sync::fuse::errors::FuseError,
};

use snafu::Snafu;

use std::io::Error as StdIoError;

use tokio::io::Error as TokioIoError;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum ServerError {
    #[snafu(display("`server` failed to initialize: {}", source))]
    InitializeFailed { source: TokioIoError },
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum ClientError {
    #[snafu(display("card is already published (shard: {:?})", shard))]
    AlreadyPublished { shard: Option<ShardId> },
    #[snafu(display("shard id is invalid"))]
    ShardIdInvalid,
    #[snafu(display("shard is full"))]
    ShardFull,
    #[snafu(display("shard is incomplete"))]
    ShardIncomplete,
    #[snafu(display("card unknown"))]
    CardUnknown,
    #[snafu(display("address unknown"))]
    AddressUnknown,
}

pub use attempt::AttemptError;
pub use listen::ListenError;
pub use serve::ServeError;

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
        ConnectionError { source: PlainConnectionError },
    }
}

pub(crate) mod attempt {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum AttemptError {
        #[snafu(display("`connect` failed: {}", source))]
        ConnectFailed { source: StdIoError },
        #[snafu(display("connection error: {}", source))]
        ConnectionError { source: PlainConnectionError },
    }
}
