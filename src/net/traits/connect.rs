use async_trait::async_trait;

use crate::net::PlainConnection;

use std::io::Result;

use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_udt::{UdtConfiguration, UdtConnection};

/*  TODO:
 * Define generic Connect
 * Retry handshake / RDV queue in UDT
*/

#[derive(Clone, Debug)]
pub enum TransportProtocol {
    TCP,
    UDT(UdtConfiguration),
}

#[derive(Clone, Debug)]
pub struct ConnectSettings {
    pub transport: TransportProtocol,
}

impl Default for ConnectSettings {
    fn default() -> Self {
        Self {
            transport: TransportProtocol::TCP,
        }
    }
}

#[async_trait]
pub trait Connect: Send + Sync {
    async fn connect(&self, settings: &ConnectSettings) -> Result<PlainConnection>;
}

#[async_trait]
impl<A> Connect for A
where
    A: Send + Sync + Clone + ToSocketAddrs,
{
    async fn connect(&self, settings: &ConnectSettings) -> Result<PlainConnection> {
        match &settings.transport {
            TransportProtocol::TCP => TcpStream::connect(self.clone()).await.and_then(|stream| {
                stream.set_nodelay(true)?;
                Ok(stream.into())
            }),
            TransportProtocol::UDT(config) => UdtConnection::connect(&self, Some(config.clone()))
                .await
                .map(Into::into),
        }
    }
}
