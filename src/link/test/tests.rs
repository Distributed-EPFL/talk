mod context {
    use crate::{
        crypto::primitives::sign::PublicKey,
        link::{
            context::{
                ConnectDispatcher, ListenDispatcher, ListenDispatcherSettings,
            },
            test::{self, System},
        },
        net::{
            test::{System as NetSystem, TestConnector},
            traits::TcpConnect,
            Connector, Listener, PlainConnection,
        },
    };

    use futures::stream::FuturesUnordered;
    use futures::StreamExt;

    #[tokio::test]
    #[should_panic(expected = "called `register` twice for the same `context`")]
    async fn connector_double_register() {
        let System { connectors, .. } = test::setup(1).await;

        let _connector_1 = connectors[0].register(format!("Context 1"));
        let _connector_2 = connectors[0].register(format!("Context 1"));
    }

    #[tokio::test]
    async fn connector_register_again() {
        let System { connectors, .. } = test::setup(1).await;

        let _connector_1 = connectors[0].register(format!("Context 1"));
        drop(_connector_1);
        let _connector_2 = connectors[0].register(format!("Context 1"));
    }

    #[tokio::test]
    #[should_panic(expected = "called `register` twice for the same `context`")]
    async fn listener_double_register() {
        let System { listeners, .. } = test::setup(1).await;

        let _listener_1 = listeners[0].register(format!("Context 1"));
        let _listener_2 = listeners[0].register(format!("Context 1"));
    }

    #[tokio::test]
    async fn listener_register_again() {
        let System { listeners, .. } = test::setup(1).await;

        let listener_1 = listeners[0].register(format!("Context 1"));
        drop(listener_1);
        let _listeners_2 = listeners[0].register(format!("Context 1"));
    }

    #[tokio::test]
    async fn simple() {
        let mut system: System = test::setup(2).await.into();

        let received = system
            .connect(0, 1, format!("Context 1"))
            .await
            .transmit(&42)
            .await;

        assert_eq!(42u32, received.unwrap());
    }

    #[tokio::test]
    async fn stress() {
        let peer = 10;

        let mut system: System = test::setup(peer).await.into();

        let mut matrix =
            system.connection_matrix(format!("Universal Context")).await;

        matrix
            .iter_mut()
            .enumerate()
            .map(|(i, column)| async move {
                column
                    .iter_mut()
                    .enumerate()
                    .map(|(j, pair)| async move {
                        let n = ((i + 1) * (j + 1)) as u32;
                        let result: u32 = pair.transmit(&n).await.unwrap();
                        assert_eq!(n, result);
                    })
                    .collect::<FuturesUnordered<_>>()
            })
            .collect::<FuturesUnordered<_>>()
            .flatten()
            .collect::<Vec<()>>()
            .await;
    }

    struct SlowConnector(TestConnector);

    impl SlowConnector {
        async fn connect(&self, root: PublicKey) -> PlainConnection {
            let address = self.0.peers.get(&root).unwrap().clone();
            address.connect().await.unwrap()

            // does not complete
        }
    }

    #[tokio::test]
    async fn slow_loris() {
        let NetSystem {
            keys,
            mut connectors,
            mut listeners,
        } = NetSystem::setup(2).await.into();

        let mut listener = ListenDispatcher::new(
            listeners.remove(1),
            ListenDispatcherSettings::default(),
        )
        .register(format!("Context"));

        let handle_a = tokio::spawn(async move {
            let _connection = listener.accept().await.unwrap();
        });

        let slow_loris = SlowConnector(connectors.remove(0));
        let _slow_connection = slow_loris.connect(keys[1]).await;

        let connector = ConnectDispatcher::new(connectors.remove(0))
            .register(format!("Context"));

        let handle_b = tokio::spawn(async move {
            let _connection = connector.connect(keys[1]).await.unwrap();
        });

        test::join([handle_a, handle_b])
            .await
            .expect("Stuck handling a slow loris");
    }
}
