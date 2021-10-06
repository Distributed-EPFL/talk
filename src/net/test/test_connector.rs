use async_trait::async_trait;

use crate::{
    crypto::{primitives::sign::PublicKey, KeyChain},
    errors::DynError,
    net::{
        test::errors::{
            test_connector::{
                AuthenticateFailed, ConnectionFailed, SecureFailed,
                UnexpectedRemote,
            },
            ConnectorError,
        },
        traits::TcpConnect,
        Connector, SecureConnection,
    },
};

use snafu::ResultExt;

use std::{collections::HashMap, net::SocketAddr};

pub struct TestConnector {
    keychain: KeyChain,
    peers: HashMap<PublicKey, SocketAddr>,
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
    ) -> Result<SecureConnection, DynError> {
        let address = self
            .peers
            .get(&root)
            .ok_or(ConnectorError::AddressUnknown)?
            .clone();

        let mut connection = address
            .connect()
            .await
            .context(ConnectionFailed)?
            .secure()
            .await
            .context(SecureFailed)?;

        let keycard = connection
            .authenticate(&self.keychain)
            .await
            .context(AuthenticateFailed)?;

        if keycard.root() == root {
            Ok(connection)
        } else {
            UnexpectedRemote {
                remote: keycard.root(),
            }
            .fail()
            .map_err(Into::into)
        }
    }
}
