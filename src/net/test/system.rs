use crate::{
    crypto::{Identity, KeyChain},
    net::{
        test::{ConnectionPair, TestConnector, TestListener},
        Connector, Listener,
    },
};
use futures::stream::{FuturesOrdered, StreamExt};
use std::{collections::HashMap, net::SocketAddr};

pub struct System {
    pub keys: Vec<Identity>,
    pub connectors: Vec<TestConnector>,
    pub listeners: Vec<TestListener>,
}

impl System {
    pub fn new(
        keys: Vec<Identity>,
        connectors: Vec<TestConnector>,
        listeners: Vec<TestListener>,
    ) -> Self {
        System {
            keys,
            connectors,
            listeners,
        }
    }

    pub async fn setup(peers: usize) -> System {
        System::setup_with_keychains((0..peers).map(|_| KeyChain::random())).await
    }

    pub async fn setup_with_keychains<I>(keychains: I) -> System
    where
        I: IntoIterator<Item = KeyChain>,
    {
        let keychains = keychains.into_iter().collect::<Vec<_>>();

        let identities = keychains
            .iter()
            .map(|keychain| keychain.keycard().identity())
            .collect::<Vec<_>>();

        let (listeners, addresses): (Vec<TestListener>, Vec<SocketAddr>) = keychains
            .iter()
            .map(|keychain| async move { TestListener::new(keychain.clone()).await })
            .collect::<FuturesOrdered<_>>()
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .unzip();

        let peers: HashMap<Identity, SocketAddr> = identities
            .clone()
            .into_iter()
            .zip(addresses.into_iter())
            .collect();

        let connectors: Vec<TestConnector> = keychains
            .into_iter()
            .map(|keychain| TestConnector::new(keychain, peers.clone()))
            .collect();

        System::new(identities, connectors, listeners)
    }

    pub async fn connect(&mut self, source: usize, destination: usize) -> ConnectionPair {
        let source_future = self.connectors[source].connect(self.keys[destination]);
        let destination_future = self.listeners[destination].accept();

        let (source, destination) = futures::join!(source_future, destination_future);

        ConnectionPair::new(source.unwrap(), destination.unwrap().1)
    }

    pub async fn connection_matrix(&mut self) -> Vec<Vec<ConnectionPair>> {
        let mut matrix = Vec::with_capacity(self.keys.len());

        for sender in 0..self.keys.len() {
            let mut row = Vec::with_capacity(self.keys.len());

            for receiver in 0..self.keys.len() {
                row.push(self.connect(sender, receiver).await);
            }

            matrix.push(row);
        }

        matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::test::join;

    #[tokio::test]
    async fn example_setup() {
        let mut system = System::setup(8).await;

        let handles = system
            .connection_matrix()
            .await
            .into_iter()
            .map(|row| {
                row.into_iter().map(|mut pair| {
                    tokio::spawn(async move {
                        let sent: u32 = 42;
                        let received: u32 = pair.transmit(&sent).await.unwrap();

                        assert_eq!(received, sent);
                    })
                })
            })
            .flatten();

        join(handles).await.unwrap();
    }
}
