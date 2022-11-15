mod unicast {
    use crate::{
        time::test::join,
        unicast::{test::UnicastSystem, Acknowledgement, PushSettings},
    };
    use futures::stream::{FuturesUnordered, StreamExt};

    #[tokio::test]
    async fn constant_one_to_one_strong() {
        let UnicastSystem {
            keys,
            mut senders,
            mut receivers,
        } = UnicastSystem::<u32>::setup(1).await.into();

        let mut receiver = receivers.remove(0);
        let sender = senders.remove(0);

        let handle = tokio::spawn(async move {
            let (_, message, acknowledger) = receiver.receive().await;

            assert_eq!(message, 42);
            acknowledger.strong();
        });

        let ack = sender.send(keys[0], 42).await.unwrap();
        assert_eq!(ack, Acknowledgement::Strong);

        join([handle]).await.unwrap();
    }

    #[tokio::test]
    async fn constant_one_to_one_strong_multiple_messages() {
        const MESSAGES: usize = 10;

        let UnicastSystem {
            keys,
            mut senders,
            mut receivers,
        } = UnicastSystem::<u32>::setup(1).await.into();

        let mut receiver = receivers.remove(0);
        let sender = senders.remove(0);

        let handle = tokio::spawn(async move {
            for _ in 0..MESSAGES {
                let (_, message, acknowledger) = receiver.receive().await;

                assert_eq!(message, 42);
                acknowledger.strong();
            }
        });

        for _ in 0..MESSAGES {
            let ack = sender.send(keys[0], 42).await.unwrap();
            assert_eq!(ack, Acknowledgement::Strong);
        }

        join([handle]).await.unwrap();
    }

    #[tokio::test]
    async fn constant_one_to_many_strong() {
        const PEERS: usize = 8;

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
                    let (_, message, acknowledger) = receiver.receive().await;

                    assert_eq!(message, 42);
                    acknowledger.strong();
                })
            })
            .collect::<Vec<_>>();

        let acknowledgements = keys
            .iter()
            .map(move |key| {
                let sender = sender.clone();
                let key = key.clone();

                async move { sender.send(key, 42).await.unwrap() }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;

        for acknowledgement in acknowledgements {
            assert_eq!(acknowledgement, Acknowledgement::Strong);
        }

        join(handles).await.unwrap();
    }

    #[tokio::test]
    async fn constant_all_to_all_strong() {
        const PEERS: usize = 8;

        let UnicastSystem {
            keys,
            senders,
            receivers,
        } = UnicastSystem::<u32>::setup(PEERS).await.into();

        let handles = receivers
            .into_iter()
            .map(|mut receiver| {
                tokio::spawn(async move {
                    for _ in 0..PEERS {
                        let (_, message, acknowledger) = receiver.receive().await;

                        assert_eq!(message, 42);
                        acknowledger.strong();
                    }
                })
            })
            .collect::<Vec<_>>();

        let acknowledgements = senders
            .iter()
            .map(|sender| {
                keys.iter().map(move |key| {
                    let sender = sender.clone();
                    let key = key.clone();

                    async move { sender.send(key, 42).await.unwrap() }
                })
            })
            .flatten()
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;

        for acknowledgement in acknowledgements {
            assert_eq!(acknowledgement, Acknowledgement::Strong);
        }

        join(handles).await.unwrap();
    }

    #[tokio::test]
    async fn constant_one_to_one_push() {
        const IGNORED: usize = 5;

        let UnicastSystem {
            keys,
            mut senders,
            mut receivers,
        } = UnicastSystem::<u32>::setup(1).await.into();

        let mut receiver = receivers.remove(0);
        let sender = senders.remove(0);

        let handle = tokio::spawn(async move {
            for _ in 0..IGNORED - 1 {
                let (_, message, _) = receiver.receive().await;
                assert_eq!(message, 42);
            }

            let (_, message, acknowledger) = receiver.receive().await;
            assert_eq!(message, 42);
            acknowledger.strong();
        });

        sender
            .push(keys[0], 42, PushSettings::strong_constant())
            .await;

        join([handle]).await.unwrap();
    }

    #[tokio::test]
    async fn constant_one_to_one_push_brief_ok() {
        let UnicastSystem {
            keys,
            mut senders,
            mut receivers,
        } = UnicastSystem::<u32>::setup(1).await.into();

        let mut receiver = receivers.remove(0);
        let sender = senders.remove(0);

        let handle = tokio::spawn(async move {
            let (_, message, acknowledger) = receiver.receive().await;

            assert_eq!(message, 42);
            acknowledger.strong();
        });

        sender
            .push_brief(keys[0], 42, 43, PushSettings::strong_constant())
            .await;

        join([handle]).await.unwrap();
    }

    #[tokio::test]
    async fn constant_one_to_one_push_brief_expand() {
        let UnicastSystem {
            keys,
            mut senders,
            mut receivers,
        } = UnicastSystem::<u32>::setup(1).await.into();

        let mut receiver = receivers.remove(0);
        let sender = senders.remove(0);

        let handle = tokio::spawn(async move {
            let (_, message, acknowledger) = receiver.receive().await;

            assert_eq!(message, 42);
            acknowledger.expand();

            let (_, message, acknowledger) = receiver.receive().await;

            assert_eq!(message, 43);
            acknowledger.strong();
        });

        sender
            .push_brief(keys[0], 42, 43, PushSettings::strong_constant())
            .await;

        join([handle]).await.unwrap();
    }
}
