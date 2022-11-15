use crate::{
    broadcast::BestEffortSettings,
    crypto::Identity,
    net::Message,
    sync::fuse::{Fuse, Relay},
    unicast::Sender,
};
use futures::stream::{FuturesUnordered, Stream, StreamExt};
use std::pin::Pin;
use tokio::task::JoinHandle;

pub struct BestEffort {
    stream: Pin<Box<dyn Stream<Item = Identity> + Send + Sync>>,
    completed: Vec<Identity>,
}

impl BestEffort {
    pub fn new<M, R>(
        sender: Sender<M>,
        remotes: R,
        message: M,
        settings: BestEffortSettings,
    ) -> Self
    where
        M: Message + Clone,
        R: IntoIterator<Item = Identity>,
    {
        BestEffort::setup(sender, remotes, message, None, settings)
    }

    pub fn brief<M, R>(
        sender: Sender<M>,
        remotes: R,
        brief: M,
        expanded: M,
        settings: BestEffortSettings,
    ) -> Self
    where
        M: Message + Clone,
        R: IntoIterator<Item = Identity>,
    {
        BestEffort::setup(sender, remotes, brief, Some(expanded), settings)
    }

    fn setup<M, R>(
        sender: Sender<M>,
        remotes: R,
        message: M,
        fallback: Option<M>,
        settings: BestEffortSettings,
    ) -> Self
    where
        M: Message + Clone,
        R: IntoIterator<Item = Identity>,
    {
        let unordered = remotes
            .into_iter()
            .map(|remote| {
                let sender = sender.clone();
                let message = message.clone();
                let fallback = fallback.clone();
                let settings = settings.clone();

                async move {
                    if let Some(fallback) = fallback {
                        sender
                            .push_brief(remote, message, fallback, settings.push_settings)
                            .await;
                    } else {
                        sender.push(remote, message, settings.push_settings).await;
                    }

                    remote
                }
            })
            .collect::<FuturesUnordered<_>>();

        let completed = Vec::with_capacity(unordered.len());
        let stream = Box::pin(unordered);

        BestEffort { stream, completed }
    }

    pub fn completed(&self) -> &[Identity] {
        self.completed.as_slice()
    }

    pub async fn until(&mut self, threshold: usize) {
        while self.completed.len() < threshold {
            self.completed.push(
                self.stream
                    .next()
                    .await
                    .expect("Called `BestEffort::until` beyond maximum number of recipients"),
            )
        }
    }

    pub async fn complete(self) {
        self.stream.collect::<Vec<_>>().await;
    }

    pub fn run(self, relay: Relay) -> JoinHandle<Option<()>> {
        relay.run(async move { self.complete().await })
    }

    pub fn spawn(self, fuse: &Fuse) -> JoinHandle<Option<()>> {
        self.run(fuse.relay())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{time::test::join, unicast::test::UnicastSystem};

    use futures::stream::{FuturesUnordered, StreamExt};

    #[tokio::test]
    async fn constant_one_broadcast() {
        const PEERS: usize = 8;
        const IGNORED: usize = 8;

        let UnicastSystem {
            keys,
            mut senders,
            receivers,
        } = UnicastSystem::<u32>::setup(PEERS).await.into();

        let sender = senders.remove(0);

        let handles = receivers
            .into_iter()
            .map(|mut receiver| {
                tokio::spawn(async move {
                    for _ in 0..IGNORED {
                        let (_, message, _) = receiver.receive().await;
                        assert_eq!(message, 42);
                    }

                    let (_, message, acknowledger) = receiver.receive().await;
                    assert_eq!(message, 42);
                    acknowledger.strong();
                })
            })
            .collect::<Vec<_>>();

        let best_effort =
            BestEffort::new(sender, keys, 42u32, BestEffortSettings::strong_constant());

        let best_effort_handle = tokio::spawn(async move { best_effort.complete().await });

        join([best_effort_handle]).await.unwrap();
        join(handles).await.unwrap();
    }

    #[tokio::test]
    async fn constant_all_broadcast() {
        const PEERS: usize = 8;
        const IGNORED: usize = 8;

        let UnicastSystem {
            keys,
            senders,
            receivers,
        } = UnicastSystem::<u32>::setup(PEERS).await.into();

        let _ = receivers
            .into_iter()
            .map(|mut receiver| {
                tokio::spawn(async move {
                    for _ in 0..IGNORED - 1 {
                        let (_, message, _) = receiver.receive().await;
                        assert_eq!(message, 42);
                    }
                    loop {
                        let (_, message, acknowledger) = receiver.receive().await;
                        assert_eq!(message, 42);
                        acknowledger.strong();
                    }
                })
            })
            .collect::<Vec<_>>();

        let best_effort_handle = tokio::spawn(async move {
            senders
                .into_iter()
                .map(|sender| {
                    let best_effort = BestEffort::new(
                        sender,
                        keys.clone(),
                        42u32,
                        BestEffortSettings::strong_constant(),
                    );
                    best_effort.complete()
                })
                .collect::<FuturesUnordered<_>>()
                .collect::<Vec<_>>()
                .await;
        });

        join([best_effort_handle]).await.unwrap();
    }

    #[tokio::test]
    #[should_panic]
    #[ignore]
    async fn constant_all_broadcast_threshold_insufficient() {
        const FAULTY: usize = 1;
        const PEERS: usize = 3 * FAULTY + 1;

        let UnicastSystem {
            keys,
            senders,
            receivers,
        } = UnicastSystem::<u32>::setup(PEERS).await.into();

        let _ = receivers
            .into_iter()
            .enumerate()
            .map(|(i, mut receiver)| {
                tokio::spawn(async move {
                    loop {
                        let (_, message, acknowledger) = receiver.receive().await;
                        assert_eq!(message, 42);
                        if i < FAULTY {
                            acknowledger.weak();
                        } else {
                            acknowledger.strong();
                        }
                    }
                })
            })
            .collect::<Vec<_>>();

        let best_effort_handle = tokio::spawn(async move {
            senders
                .into_iter()
                .map(|sender| {
                    let keys = keys.clone();

                    async move {
                        let mut best_effort = BestEffort::new(
                            sender,
                            keys,
                            42u32,
                            BestEffortSettings::strong_constant(),
                        );
                        best_effort.until(PEERS - FAULTY + 1).await;
                    }
                })
                .collect::<FuturesUnordered<_>>()
                .collect::<Vec<_>>()
                .await;
        });

        join([best_effort_handle]).await.unwrap();
    }

    #[tokio::test]
    async fn constant_all_broadcast_threshold_sufficient() {
        const FAULTY: usize = 1;
        const PEERS: usize = 3 * FAULTY + 1;

        let UnicastSystem {
            keys,
            senders,
            receivers,
        } = UnicastSystem::<u32>::setup(PEERS).await.into();

        let _ = receivers
            .into_iter()
            .enumerate()
            .map(|(i, mut receiver)| {
                tokio::spawn(async move {
                    loop {
                        let (_, message, acknowledger) = receiver.receive().await;
                        assert_eq!(message, 42);
                        if i < FAULTY {
                            acknowledger.weak();
                        } else {
                            acknowledger.strong();
                        }
                    }
                })
            })
            .collect::<Vec<_>>();

        let best_effort_handle = tokio::spawn(async move {
            senders
                .into_iter()
                .map(|sender| {
                    let keys = keys.clone();

                    async move {
                        let mut best_effort = BestEffort::new(
                            sender,
                            keys,
                            42u32,
                            BestEffortSettings::strong_constant(),
                        );
                        best_effort.until(PEERS - FAULTY).await;
                    }
                })
                .collect::<FuturesUnordered<_>>()
                .collect::<Vec<_>>()
                .await;
        });

        join([best_effort_handle]).await.unwrap();
    }
}
