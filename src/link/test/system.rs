use crate::{
    crypto::primitives::sign::PublicKey,
    link::context::{
        ConnectDispatcher, ListenDispatcher, ListenDispatcherSettings,
    },
    net::test,
};

pub async fn setup(
    peers: usize,
) -> (
    Vec<PublicKey>,
    Vec<ConnectDispatcher>,
    Vec<ListenDispatcher>,
) {
    let (keys, connectors, listeners) = test::setup(peers).await;

    let connectors = connectors
        .into_iter()
        .map(|connector| ConnectDispatcher::new(connector))
        .collect();
    let listeners = listeners
        .into_iter()
        .map(|connector| {
            ListenDispatcher::new(
                connector,
                ListenDispatcherSettings::default(),
            )
        })
        .collect();

    (keys, connectors, listeners)
}
