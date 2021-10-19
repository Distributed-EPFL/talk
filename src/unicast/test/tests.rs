mod unicast {
    use crate::{time::test::join, unicast::{Acknowledgement, acknowledgement, test::UnicastSystem}};

    use futures::stream::{FuturesUnordered, StreamExt};

    #[tokio::test]
    async fn constant_all_to_all_strong() {
        const PEERS: usize = 8;

        let UnicastSystem {
            keys,
            senders,
            receivers,
        } = UnicastSystem::<u32>::setup(PEERS).await.into();

        let handles = receivers.into_iter().map(|mut receiver| {
            tokio::spawn(async move {
                for _ in 0..PEERS {
                    let (_, message, acknowledger) = receiver.receive().await;

                    assert_eq!(message, 42);
                    acknowledger.strong();
                }
            })
        });

        let acknowledgements = senders
            .iter()
            .map(|sender| {
                keys.iter().map(move |key| {
                    let sender = sender.clone();
                    let key = key.clone();

                    async move {
                        sender.send(key, 42).await.unwrap()
                    }
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
}
