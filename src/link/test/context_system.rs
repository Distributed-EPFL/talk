use crate::{
    crypto::Identity,
    link::context::{ConnectDispatcher, ContextId, ListenDispatcher},
    net::{
        test::{ConnectionPair, System as NetSystem},
        Connector, Listener,
    },
};

pub struct ContextSystem {
    pub keys: Vec<Identity>,
    pub connectors: Vec<ConnectDispatcher>,
    pub listeners: Vec<ListenDispatcher>,
}

impl ContextSystem {
    pub fn new(
        keys: Vec<Identity>,
        connectors: Vec<ConnectDispatcher>,
        listeners: Vec<ListenDispatcher>,
    ) -> Self {
        ContextSystem {
            keys,
            connectors,
            listeners,
        }
    }

    pub async fn setup(peers: usize) -> ContextSystem {
        let NetSystem {
            keys,
            connectors,
            listeners,
        } = NetSystem::setup(peers).await;

        let connectors = connectors
            .into_iter()
            .map(|connector| ConnectDispatcher::new(connector))
            .collect();

        let listeners = listeners
            .into_iter()
            .map(|listener| ListenDispatcher::new(listener, Default::default()))
            .collect();

        ContextSystem::new(keys, connectors, listeners)
    }

    pub async fn connect(
        &mut self,
        source: usize,
        destination: usize,
        context: ContextId,
    ) -> ConnectionPair {
        let connector = self.connectors[source].register(context.clone());
        let mut listener = self.listeners[destination].register(context);

        let source_future = connector.connect(self.keys[destination]);
        let destination_future = listener.accept();

        let (source, destination) =
            futures::join!(source_future, destination_future);

        ConnectionPair::new(source.unwrap(), destination.unwrap().1)
    }

    pub async fn connection_matrix(
        &mut self,
        context: ContextId,
    ) -> Vec<Vec<ConnectionPair>> {
        let mut matrix = Vec::with_capacity(self.keys.len());

        for sender in 0..self.keys.len() {
            let mut row = Vec::with_capacity(self.keys.len());

            for receiver in 0..self.keys.len() {
                row.push(self.connect(sender, receiver, context.clone()).await);
            }

            matrix.push(row);
        }

        matrix
    }
}
