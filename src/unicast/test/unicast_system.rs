use crate::{
    crypto::Identity,
    net::test::System as NetSystem,
    unicast::{Message as UnicastMessage, Receiver, Sender},
};

pub struct UnicastSystem<Message: UnicastMessage> {
    pub keys: Vec<Identity>,
    pub senders: Vec<Sender<Message>>,
    pub receivers: Vec<Receiver<Message>>,
}

impl<Message> UnicastSystem<Message>
where
    Message: UnicastMessage,
{
    pub fn new(
        keys: Vec<Identity>,
        senders: Vec<Sender<Message>>,
        receivers: Vec<Receiver<Message>>,
    ) -> Self {
        UnicastSystem {
            keys,
            senders,
            receivers,
        }
    }

    pub async fn setup(peers: usize) -> UnicastSystem<Message> {
        let NetSystem {
            keys,
            connectors,
            listeners,
        } = NetSystem::setup(peers).await;

        let senders = connectors
            .into_iter()
            .map(|connector| Sender::new(connector, Default::default()))
            .collect::<Vec<_>>();

        let receivers = listeners
            .into_iter()
            .map(|listener| Receiver::new(listener, Default::default()))
            .collect::<Vec<_>>();

        UnicastSystem::new(keys, senders, receivers)
    }
}
