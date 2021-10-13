use crate::{
    crypto::{primitives::sign::PublicKey, KeyChain},
    net::{
        test::{self, ConnectionPair, TestConnector, TestListener},
        Connector, Listener,
    },
};

use futures::stream::{FuturesOrdered, StreamExt};

use std::collections::HashMap;
use std::net::SocketAddr;

pub(crate) struct System {
    pub keys: Vec<PublicKey>,
    pub connectors: Vec<TestConnector>,
    pub listeners: Vec<TestListener>,
}

impl System {
    pub(crate) async fn setup(peers: usize) -> System {
        let keychains =
            (0..peers).map(|_| KeyChain::random()).collect::<Vec<_>>();

        let roots = keychains
            .iter()
            .map(|keychain| keychain.keycard().root())
            .collect::<Vec<_>>();

        let (listeners, addresses): (Vec<TestListener>, Vec<SocketAddr>) =
            keychains
                .iter()
                .map(|keychain| async move {
                    TestListener::new(keychain.clone()).await
                })
                .collect::<FuturesOrdered<_>>()
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .unzip();

        let peers: HashMap<PublicKey, SocketAddr> = roots
            .clone()
            .into_iter()
            .zip(addresses.into_iter())
            .collect();

        let connectors: Vec<TestConnector> = keychains
            .into_iter()
            .map(|keychain| TestConnector::new(keychain, peers.clone()))
            .collect();

        System::new(roots, connectors, listeners)
    }

    pub(crate) fn new(
        keys: Vec<PublicKey>,
        connectors: Vec<TestConnector>,
        listeners: Vec<TestListener>,
    ) -> Self {
        System {
            keys,
            connectors,
            listeners,
        }
    }

    pub(crate) async fn connect(
        &mut self,
        peer_a: usize,
        peer_b: usize,
    ) -> ConnectionPair {
        let remote = self.keys[peer_b].clone();

        let fut_a = self.connectors[peer_a].connect(remote);
        let fut_b = self.listeners[peer_b].accept();

        let (connection_a, connection_b) = futures::join!(fut_a, fut_b);

        ConnectionPair::new(connection_a.unwrap(), connection_b.unwrap().1)
    }

    pub(crate) async fn connection_matrix(
        &mut self,
    ) -> Vec<Vec<ConnectionPair>> {
        let mut matrix = Vec::with_capacity(self.keys.len());
        for sender in 0..self.keys.len() {
            let mut column = Vec::with_capacity(self.keys.len());
            for receiver in 0..self.keys.len() {
                column.push(self.connect(sender, receiver).await);
            }
            matrix.push(column);
        }
        matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        net::{Connector, Listener},
        sync::fuse::Fuse,
    };

    #[tokio::test]
    async fn example_setup() {
        let peers = 10;

        let System {
            keys,
            connectors,
            listeners,
        } = System::setup(peers).await;

        let fuse = Fuse::new();

        let handles = listeners
            .into_iter()
            .map(|mut listener| {
                let mut relay = fuse.relay();

                tokio::spawn(async move {
                    for _ in 0..peers {
                        let (_, mut connection) = relay
                            .map(listener.accept())
                            .await
                            .unwrap()
                            .unwrap();

                        let message: u32 = connection.receive().await.unwrap();

                        connection
                            .send(&(message + peers as u32))
                            .await
                            .unwrap();
                    }
                })
            })
            .collect::<Vec<_>>();

        for sender in 0..peers {
            for receiver in 0..peers {
                let mut connection =
                    connectors[sender].connect(keys[receiver]).await.unwrap();

                connection.send(&(sender as u32)).await.unwrap();
                let response: u32 = connection.receive().await.unwrap();

                assert_eq!(response, (sender + peers) as u32);
            }
        }

        test::join(handles).await.unwrap();
    }
}
