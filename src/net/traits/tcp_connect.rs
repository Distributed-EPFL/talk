use crate::net::PlainConnection;
use async_trait::async_trait;
use std::io::Result;
use tokio::net::{TcpStream, ToSocketAddrs};

#[async_trait]
pub trait TcpConnect: Send + Sync {
    async fn connect(&self) -> Result<PlainConnection>;
}

#[async_trait]
impl<A> TcpConnect for A
where
    A: Send + Sync + Clone + ToSocketAddrs,
{
    async fn connect(&self) -> Result<PlainConnection> {
        TcpStream::connect(self.clone()).await.and_then(|stream| {
            stream.set_nodelay(true)?;
            Ok(stream.into())
        })
    }
}
