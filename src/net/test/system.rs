use crate::{
    crypto::{primitives::sign::PublicKey, KeyChain},
    net::test::{TestConnector, TestListener},
};

use futures::stream::{FuturesOrdered, FuturesUnordered, StreamExt};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time;

const TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn setup(
    peers: usize,
) -> (Vec<PublicKey>, Vec<TestConnector>, Vec<TestListener>) {
    let keychains = (0..peers).map(|_| KeyChain::random()).collect::<Vec<_>>();

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

    (roots, connectors, listeners)
}

pub(crate) async fn join<I, T>(handles: I)
where
    I: IntoIterator<Item = JoinHandle<T>>,
{
    if time::timeout(
        TIMEOUT,
        handles
            .into_iter()
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>(),
    )
    .await
    .is_err()
    {
        panic!("`join` failed: unfinished tasks remaining");
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
        let peers = 8;
        let (keys, connectors, listeners) = setup(peers).await;

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

        join(handles).await;
    }
}
