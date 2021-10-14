use crate::{
    crypto::primitives::sign::PublicKey,
    link::context::{
        ConnectDispatcher, ContextId, ListenDispatcher,
    },
    net::{
        test::{ConnectionPair, System as NetSystem},
        Connector, Listener,
    },
};

pub(crate) struct ContextSystem {
    pub keys: Vec<PublicKey>,
    pub connectors: Vec<ConnectDispatcher>,
    pub listeners: Vec<ListenDispatcher>,
}

impl ContextSystem {
    pub(crate) fn new(
        keys: Vec<PublicKey>,
        connectors: Vec<ConnectDispatcher>,
        listeners: Vec<ListenDispatcher>,
    ) -> Self {
        ContextSystem {
            keys,
            connectors,
            listeners,
        }
    }

    pub(crate) async fn setup(peers: usize) -> ContextSystem {
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
            .map(|listener| {
                ListenDispatcher::new(
                    listener,
                    Default::default(),
                )
            })
            .collect();

        ContextSystem::new(keys, connectors, listeners)
    }

    pub(crate) async fn connect(
        &mut self,
        peer_a: usize,
        peer_b: usize,
        context: ContextId,
    ) -> ConnectionPair {
        let connector = self.connectors[peer_a].register(context.clone());
        let mut listener = self.listeners[peer_b].register(context);

        let fut_a = connector.connect(self.keys[peer_b]);
        let fut_b = listener.accept();

        let (connection_a, connection_b) = futures::join!(fut_a, fut_b);

        ConnectionPair::new(connection_a.unwrap(), connection_b.unwrap().1)
    }

    pub(crate) async fn connection_matrix(
        &mut self,
        context: ContextId,
    ) -> Vec<Vec<ConnectionPair>> {
        let mut matrix = Vec::with_capacity(self.keys.len());
        for sender in 0..self.keys.len() {
            let mut column = Vec::with_capacity(self.keys.len());
            for receiver in 0..self.keys.len() {
                column.push(
                    self.connect(sender, receiver, context.clone()).await,
                );
            }
            matrix.push(column);
        }
        matrix
    }
}
