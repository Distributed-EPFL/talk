use async_trait::async_trait;

use crate::net::PlainConnection;

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
        TcpStream::connect(self.clone()).await.map(Into::into)
    }
}
