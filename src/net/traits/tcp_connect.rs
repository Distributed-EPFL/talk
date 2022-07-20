use async_trait::async_trait;

use crate::net::PlainConnection;

use std::io::Result;

use tokio::net::{lookup_host, TcpStream, ToSocketAddrs};

use tokio_udt::{UdtConfiguration, UdtConnection};

// TODO: Define generic Connect
// pub enum TransportType {
//     TCP,
//     UDT,
// }

// struct ConnectSettings {
//     transport: // TCP / UDT
// }

// #[async_trait]
// pub trait Connect: Send + Sync {
//     async fn connect(&self, settings: ConnectSettings) -> Result<PlainConnection>;
// }

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
        // TcpStream::connect(self.clone()).await.and_then(|stream| {
        //     stream.set_nodelay(true)?;
        //     Ok(stream.into())
        // })

        let addr = lookup_host(self).await?.next().expect("no addr found");
        UdtConnection::connect(
            addr,
            Some(UdtConfiguration {
                reuse_mux: false,
                ..Default::default()
            }),
        )
        .await
        .map(Into::into)
    }
}
