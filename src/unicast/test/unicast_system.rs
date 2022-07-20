use crate::{
    crypto::{Identity, KeyChain},
    net::{test::System as NetSystem, Message as NetMessage},
    unicast::{Receiver, Sender},
};

pub struct UnicastSystem<Message: NetMessage> {
    pub keys: Vec<Identity>,
    pub senders: Vec<Sender<Message>>,
    pub receivers: Vec<Receiver<Message>>,
}

impl<Message> UnicastSystem<Message>
where
    Message: NetMessage,
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
        UnicastSystem::setup_with_keychains((0..peers).map(|_| KeyChain::random())).await
    }

    pub async fn setup_with_keychains<I>(keychains: I) -> UnicastSystem<Message>
    where
        I: IntoIterator<Item = KeyChain>,
    {
        let NetSystem {
            keys,
            connectors,
            listeners,
        } = NetSystem::setup_with_keychains(keychains).await;

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
