use async_trait::async_trait;

use crate::{
    crypto::{primitives::sign::PublicKey, KeyChain},
    link::rendezvous::{Client, ConnectorSettings},
    net::{traits::TcpConnect, Connector as NetConnector, SecureConnection},
};

use doomstack::{here, Doom, ResultExt, Stack, Top};

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

pub struct Connector {
    client: Client,
    keychain: KeyChain,
    database: Arc<Mutex<Database>>,
}

struct Database {
    cache: HashMap<PublicKey, SocketAddr>,
}

#[derive(Doom)]
pub enum ConnectorError {
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

impl Connector {
    pub fn new<S>(
        server: S,
        keychain: KeyChain,
        settings: ConnectorSettings,
    ) -> Self
    where
        S: 'static + TcpConnect,
    {
        let client = Client::new(server, settings.client_settings);

        let database = Arc::new(Mutex::new(Database {
            cache: HashMap::new(),
        }));

        Connector {
            client,
            keychain,
            database,
        }
    }

    async fn attempt(
        &self,
        root: PublicKey,
    ) -> Result<SecureConnection, Top<ConnectorError>> {
        let address = self
            .get_address(root)
            .ok_or(ConnectorError::AddressUnknown.into_top())
            .spot(here!())?;

        let mut connection = address
            .connect()
            .await
            .map_err(ConnectorError::connect_failed)
            .map_err(Doom::into_top)
            .spot(here!())?
            .secure()
            .await
            .pot(ConnectorError::SecureFailed, here!())?;

        let keycard = connection
            .authenticate(&self.keychain)
            .await
            .pot(ConnectorError::AuthenticateFailed, here!())?;

        if keycard.root() == root {
            Ok(connection)
        } else {
            ConnectorError::UnexpectedRemote {
                remote: keycard.root(),
            }
            .fail()
            .spot(here!())
        }
    }

    async fn refresh(&self, root: PublicKey) -> bool {
        let stale = self.get_address(root);
        let fresh = self.client.get_address(root).await.ok().or(stale.clone());

        if fresh != stale {
            self.cache_address(root, fresh.unwrap()); // `fresh` can be `None` only if `stale` is `None` too
            true
        } else {
            false
        }
    }

    fn get_address(&self, root: PublicKey) -> Option<SocketAddr> {
        self.database
            .lock()
            .unwrap()
            .cache
            .get(&root)
            .map(Clone::clone)
    }

    fn cache_address(&self, root: PublicKey, address: SocketAddr) {
        self.database.lock().unwrap().cache.insert(root, address);
    }
}

#[async_trait]
impl NetConnector for Connector {
    async fn connect(
        &self,
        root: PublicKey,
    ) -> Result<SecureConnection, Stack> {
        loop {
            let result = self.attempt(root).await.map_err(Into::into);

            if result.is_ok() || !self.refresh(root).await {
                return result;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        link::rendezvous::{Listener, Server},
        net::Listener as NetListener,
    };

    #[tokio::test]
    async fn connect() {
        const SERVER: &str = "127.0.0.1:1250";
        const MESSAGE: &str = "Hello Alice, this is Bob!";

        let _server = Server::new(SERVER, Default::default()).await.unwrap();

        let alice_keychain = KeyChain::random();
        let bob_keychain = KeyChain::random();

        let alice_public = alice_keychain.keycard().root();
        let bob_public = bob_keychain.keycard().root();

        let mut alice_listener =
            Listener::new(SERVER, alice_keychain, Default::default()).await;

        let bob_connector =
            Connector::new(SERVER, bob_keychain, Default::default());

        let alice_task = tokio::spawn(async move {
            let (remote, mut connection) =
                alice_listener.accept().await.unwrap();

            assert_eq!(remote, bob_public);
            assert_eq!(connection.receive::<String>().await.unwrap(), MESSAGE);
        });

        let mut connection = bob_connector.connect(alice_public).await.unwrap();
        connection.send(&String::from(MESSAGE)).await.unwrap();

        alice_task.await.unwrap();
    }
}
