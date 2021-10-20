use crate::{
    broadcast::BestEffortSettings,
    crypto::primitives::sign::PublicKey,
    unicast::{Message, Sender},
};

use futures::stream::{FuturesUnordered, Stream, StreamExt};

use std::pin::Pin;

pub struct BestEffort {
    stream: Pin<Box<dyn Stream<Item = PublicKey> + Send + Sync>>,
    completed: Vec<PublicKey>,
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
        R: IntoIterator<Item = PublicKey>,
    {
        let unordered = remotes
            .into_iter()
            .map(|remote| {
                let sender = sender.clone();
                let message = message.clone();
                let settings = settings.clone();

                async move {
                    sender.push(remote, message, settings.push_settings).await;
                    remote
                }
            })
            .collect::<FuturesUnordered<_>>();

        let completed = Vec::with_capacity(unordered.len());
        let stream = Box::pin(unordered);

        BestEffort { stream, completed }
    }

    pub fn completed(&self) -> &[PublicKey] {
        self.completed.as_slice()
    }

    pub async fn until(&mut self, threshold: usize) {
        while self.completed.len() < threshold {
            self.completed.push(
                self.stream.next().await.expect(
                    "Called `BestEffort::until` beyond maximum number of recipients",
                ),
            )
        }
    }

    pub async fn complete(self) {
        self.stream.collect::<Vec<_>>().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        time::{sleep_schedules::Constant, test::join},
        unicast::{test::UnicastSystem, Acknowledgement},
    };

    use futures::stream::{FuturesUnordered, StreamExt};

    use std::sync::Arc;
    use std::time::Duration;

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

        let mut settings = BestEffortSettings::default();
        settings.push_settings.retry_schedule =
            Arc::new(Constant::new(Duration::from_millis(100)));
        settings.push_settings.stop_condition = Acknowledgement::Strong;

        let best_effort = BestEffort::new(sender, keys, 42u32, settings);

        let best_effort_handle =
            tokio::spawn(async move { best_effort.complete().await });

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
                        let (_, message, acknowledger) =
                            receiver.receive().await;
                        assert_eq!(message, 42);
                        acknowledger.strong();
                    }
                })
            })
            .collect::<Vec<_>>();

        let mut settings = BestEffortSettings::default();
        settings.push_settings.retry_schedule =
            Arc::new(Constant::new(Duration::from_millis(100)));
        settings.push_settings.stop_condition = Acknowledgement::Strong;

        let best_effort_handle = tokio::spawn(async move {
            senders
                .into_iter()
                .map(|sender| {
                    let best_effort = BestEffort::new(
                        sender,
                        keys.clone(),
                        42u32,
                        settings.clone(),
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
                        let (_, message, acknowledger) =
                            receiver.receive().await;
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

        let mut settings = BestEffortSettings::default();
        settings.push_settings.retry_schedule =
            Arc::new(Constant::new(Duration::from_millis(100)));
        settings.push_settings.stop_condition = Acknowledgement::Strong;

        let best_effort_handle = tokio::spawn(async move {
            senders
                .into_iter()
                .map(|sender| {
                    let keys = keys.clone();
                    let settings = settings.clone();

                    async move {
                        let mut best_effort =
                            BestEffort::new(sender, keys, 42u32, settings);
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
                        let (_, message, acknowledger) =
                            receiver.receive().await;
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

        let mut settings = BestEffortSettings::default();
        settings.push_settings.retry_schedule =
            Arc::new(Constant::new(Duration::from_millis(100)));
        settings.push_settings.stop_condition = Acknowledgement::Strong;

        let best_effort_handle = tokio::spawn(async move {
            senders
                .into_iter()
                .map(|sender| {
                    let keys = keys.clone();
                    let settings = settings.clone();

                    async move {
                        let mut best_effort =
                            BestEffort::new(sender, keys, 42u32, settings);
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
