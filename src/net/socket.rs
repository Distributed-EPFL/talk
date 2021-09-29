use tokio::io::{AsyncRead, AsyncWrite};

/// A `Socket` is able to asynchronously send and receive data from
/// the network.
pub trait Socket: AsyncRead + AsyncWrite + Send + Sync + Unpin {}
