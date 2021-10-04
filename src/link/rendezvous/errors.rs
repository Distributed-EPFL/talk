use crate::{
    crypto::primitives::sign::PublicKey,
    link::rendezvous::ShardId,
    net::errors::{PlainConnectionError, SecureConnectionError},
    sync::fuse::errors::FuseError,
};

use snafu::Snafu;

use std::io::Error as StdIoError;

use tokio::io::Error as TokioIoError;

pub use client::ClientError;
pub use connector::ConnectorError;
pub use server::ServerError;

pub(crate) mod server {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum ServerError {
        #[snafu(display("`server` failed to initialize: {}", source))]
        InitializeFailed { source: TokioIoError },
    }

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum ListenError {
        #[snafu(display("`listen` interrupted: {}", source))]
        ListenInterrupted { source: FuseError },
    }

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum ServeError {
        #[snafu(display("`serve` interrupted: {}", source))]
        ServeInterrupted { source: FuseError },
        #[snafu(display("connection error: {}", source))]
        ConnectionError { source: PlainConnectionError },
    }
}

pub(crate) mod client {
    use super::*;

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

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum AttemptError {
        #[snafu(display("`connect` failed: {}", source))]
        ConnectFailed { source: StdIoError },
        #[snafu(display("connection error: {}", source))]
        ConnectionError { source: PlainConnectionError },
    }
}

pub(crate) mod listener {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum ListenError {
        #[snafu(display("`listen` interrupted: {}", source))]
        ListenInterrupted { source: FuseError },
    }
}

pub(crate) mod connector {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum ConnectorError {
        #[snafu(display("address unknown"))]
        AddressUnknown,
        #[snafu(display("connection failed: {}", source))]
        ConnectionFailed { source: StdIoError },
        #[snafu(display("`secure` failed: {}", source))]
        SecureFailed { source: SecureConnectionError },
        #[snafu(display("`authenticate` failed: {}", source))]
        AuthenticateFailed { source: SecureConnectionError },
        #[snafu(display("unexpected remote: {:?}", remote))]
        UnexpectedRemote { remote: PublicKey },
    }
}
