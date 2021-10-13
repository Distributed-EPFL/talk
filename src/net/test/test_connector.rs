use async_trait::async_trait;

use crate::{
    crypto::{primitives::sign::PublicKey, KeyChain},
    net::{traits::TcpConnect, Connector, SecureConnection},
};

use doomstack::{here, Doom, ResultExt, Stack};

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;

pub(crate) struct TestConnector {
    pub keychain: KeyChain,
    pub peers: HashMap<PublicKey, SocketAddr>,
}

#[derive(Doom)]
pub enum TestConnectorError {
    #[doom(description("Address unknown"))]
    AddressUnknown,
    #[doom(description("Failed to `authenticate` connection"))]
    AuthenticateFailed,
    #[doom(description("Failed to connect: {}", source))]
    #[doom(wrap(connect_failed))]
    ConnectFailed { source: io::Error },
    #[doom(description("Failed to `secure` connection"))]
    SecureFailed,
    #[doom(description("Unexpected remote: {:?}", remote))]
    UnexpectedRemote { remote: PublicKey },
}

impl TestConnector {
    pub fn new(
        keychain: KeyChain,
        peers: HashMap<PublicKey, SocketAddr>,
    ) -> Self {
        TestConnector { keychain, peers }
    }
}

#[async_trait]
impl Connector for TestConnector {
    async fn connect(
        &self,
        root: PublicKey,
    ) -> Result<SecureConnection, Stack> {
        let address = self
            .peers
            .get(&root)
            .ok_or(TestConnectorError::AddressUnknown.into_stack())
            .spot(here!())?
            .clone();

        let mut connection = address
            .connect()
            .await
            .map_err(TestConnectorError::connect_failed)
            .map_err(Doom::into_top)
            .spot(here!())?
            .secure()
            .await
            .pot(TestConnectorError::SecureFailed, here!())?;

        let keycard = connection
            .authenticate(&self.keychain)
            .await
            .pot(TestConnectorError::AuthenticateFailed, here!())?;

        if keycard.root() == root {
            Ok(connection)
        } else {
            TestConnectorError::UnexpectedRemote {
                remote: keycard.root(),
            }
            .fail()
            .spot(here!())
            .map_err(Into::into)
        }
    }
}
