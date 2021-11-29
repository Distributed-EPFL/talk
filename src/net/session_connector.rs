use crate::{
    crypto::Identity,
    net::{Connector as NetConnector, SecureConnection, Session, SessionControl},
    sync::{fuse::Fuse, lenders::AtomicLender},
};

use doomstack::{here, Doom, ResultExt, Stack, Top};

use parking_lot::Mutex;

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    time,
};

type ConnectionInlet = Sender<(Identity, SecureConnection)>;
type ConnectionOutlet = Receiver<(Identity, SecureConnection)>;

pub struct SessionConnector {
    connector: Arc<dyn NetConnector>,
    pool: Arc<Mutex<Pool>>,
    return_inlet: ConnectionInlet,
    _fuse: Fuse,
}

struct Pool {
    connections: HashMap<Identity, Vec<Arc<AtomicLender<State>>>>,
}

enum State {
    Healthy(SecureConnection),
    Broken,
}

#[derive(Doom)]
enum KeepAliveError {
    #[doom(description("Connection error"))]
    ConnectionError,
    #[doom(description("Unexpected response"))]
    UnexpectedResponse,
    #[doom(description("Unused connection timed out"))]
    Timeout,
}

impl SessionConnector {
    pub fn new<C>(connector: C) -> Self
    where
        C: NetConnector,
    {
        let connector = Arc::new(connector);

        let pool = Arc::new(Mutex::new(Pool {
            connections: HashMap::new(),
        }));

        let (return_inlet, return_outlet) = mpsc::channel(32); // TODO: Add settings
        let fuse = Fuse::new();

        {
            let pool = pool.clone();

            fuse.spawn(async move {
                SessionConnector::handle_returns(pool, return_outlet).await;
            });
        }

        SessionConnector {
            connector,
            pool,
            return_inlet,
            _fuse: fuse,
        }
    }

    pub async fn connect(&self, remote: Identity) -> Result<Session, Stack> {
        let connection = {
            let mut pool = self.pool.lock();

            // Try to get a `Healthy` connection from `pool`
            pool.connections
                .get_mut(&remote)
                .map(|states| {
                    // This contains the `State`s which could not be `try_take`n
                    // because they're currently being pinged by `keep_alive`
                    let mut restore = Vec::new();

                    let connection = loop {
                        let state = match states.pop() {
                            Some(state) => state,
                            None => break None, // `states` exhausted, no connection available
                        };

                        match state.try_take() {
                            Some(State::Healthy(connection)) => break Some(connection),
                            Some(State::Broken) => {} // If `state` is `Broken`, garbage collect
                            None => restore.push(state), // Currently pinging, store in `restore` (see above)
                        }
                    };

                    // Flush `restore` back in `states`
                    states.extend(restore);
                    connection
                })
                .flatten()
        };

        let connection = if let Some(mut connection) = connection {
            connection.send_raw(&SessionControl::Connect).await?;
            connection
        } else {
            self.connector.connect(remote).await?
        };

        Ok(Session::new(remote, connection, self.return_inlet.clone()))
    }

    async fn handle_returns(pool: Arc<Mutex<Pool>>, mut return_outlet: ConnectionOutlet) {
        let fuse = Fuse::new();

        loop {
            if let Some((remote, connection)) = return_outlet.recv().await {
                let state = Arc::new(AtomicLender::new(State::Healthy(connection)));

                {
                    let state = state.clone();
                    let mut pool = pool.lock();

                    pool.connections
                        .entry(remote)
                        .or_insert(Vec::new())
                        .push(state);
                }

                fuse.spawn(async move {
                    let _ = SessionConnector::keep_alive(state).await;
                });
            }
        }
    }

    async fn keep_alive(state: Arc<AtomicLender<State>>) -> Result<(), Top<KeepAliveError>> {
        let start = Instant::now();

        loop {
            time::sleep(Duration::from_secs(30)).await; // TODO: Add settings

            let mut connection = if let Some(state) = state.try_take() {
                match state {
                    State::Healthy(connection) => connection,
                    State::Broken => unreachable!(), // Only `keep_alive` sets `state` to `Broken`
                }
            } else {
                // `SessionConnector::connect` is the only other function that
                // can `take` a connection out of `state`: `connection` is currently
                // in a new `Session`, keepalives are no longer necessary
                return Ok(());
            };

            if start.elapsed() > Duration::from_secs(1200) {
                state.restore(State::Broken);
                return KeepAliveError::Timeout.fail().spot(here!());
            }

            let result = async {
                connection
                    .send_raw(&SessionControl::KeepAlive)
                    .await
                    .pot(KeepAliveError::ConnectionError, here!())?;

                match connection
                    .receive()
                    .await
                    .pot(KeepAliveError::ConnectionError, here!())?
                {
                    SessionControl::KeepAlive => {}
                    _ => {
                        return KeepAliveError::UnexpectedResponse.fail().spot(here!());
                    }
                }

                Ok(connection)
            }
            .await;

            match result {
                Ok(connection) => {
                    state.restore(State::Healthy(connection));
                }
                Err(error) => {
                    state.restore(State::Broken);
                    return Err(error);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::net::{test::System, SessionListener};

    use futures::stream::{FuturesUnordered, StreamExt};

    #[tokio::test]
    async fn single() {
        let System {
            mut connectors,
            mut listeners,
            keys,
        } = System::setup(2).await;

        let connector = SessionConnector::new(connectors.remove(0));
        let mut listener = SessionListener::new(listeners.remove(1));

        tokio::spawn(async move {
            let (_, mut session) = listener.accept().await;
            assert_eq!(session.receive::<u32>().await.unwrap(), 42u32);
            session.send(&43u32).await.unwrap();
            session.end();
        });

        let mut session = connector.connect(keys[1]).await.unwrap();
        session.send(&42u32).await.unwrap();
        assert_eq!(session.receive::<u32>().await.unwrap(), 43u32);
        session.end();
    }

    #[tokio::test]
    async fn sequence() {
        let System {
            mut connectors,
            mut listeners,
            keys,
        } = System::setup(2).await;

        let connector = SessionConnector::new(connectors.remove(0));
        let mut listener = SessionListener::new(listeners.remove(1));

        tokio::spawn(async move {
            for _ in 0..10 {
                let (_, mut session) = listener.accept().await;
                assert_eq!(session.receive::<u32>().await.unwrap(), 42u32);
                session.send(&43u32).await.unwrap();
                session.end();
            }
        });

        for _ in 0..10 {
            let mut session = connector.connect(keys[1]).await.unwrap();
            session.send(&42u32).await.unwrap();
            assert_eq!(session.receive::<u32>().await.unwrap(), 43u32);
            session.end();
        }
    }

    #[tokio::test]
    async fn sequence_reuse() {
        let System {
            mut connectors,
            mut listeners,
            keys,
        } = System::setup(2).await;

        let connector = SessionConnector::new(connectors.remove(0));
        let mut listener = SessionListener::new(listeners.remove(1));

        tokio::spawn(async move {
            for _ in 0..10 {
                let (_, mut session) = listener.accept().await;
                assert_eq!(session.receive::<u32>().await.unwrap(), 42u32);
                session.send(&43u32).await.unwrap();
                session.end();
            }
        });

        for _ in 0..10 {
            {
                let mut session = connector.connect(keys[1]).await.unwrap();
                session.send(&42u32).await.unwrap();
                assert_eq!(session.receive::<u32>().await.unwrap(), 43u32);
                session.end();
            }

            time::sleep(Duration::from_millis(10)).await;
        }

        for connections in connector.pool.lock().connections.values() {
            assert_eq!(connections.len(), 1);
        }
    }

    #[tokio::test]
    async fn all_to_one() {
        let System {
            connectors,
            mut listeners,
            keys,
        } = System::setup(10).await;

        let connectors = connectors
            .into_iter()
            .map(|connector| SessionConnector::new(connector))
            .collect::<Vec<_>>();

        let mut listener = SessionListener::new(listeners.remove(0));

        tokio::spawn(async move {
            for _ in 0..(10 * 10) {
                let (id, mut session) = listener.accept().await;
                assert_eq!(session.receive::<u32>().await.unwrap(), 42u32);
                session.send(&id).await.unwrap();
                session.end();
            }
        });

        let _ = connectors
            .into_iter()
            .zip(keys.clone())
            .map(|(connector, identity)| {
                let remote = keys[0].clone();

                async move {
                    for _ in 0..10 {
                        {
                            let mut session = connector.connect(remote).await.unwrap();
                            session.send(&42u32).await.unwrap();
                            assert_eq!(session.receive::<Identity>().await.unwrap(), identity);
                            session.end();
                        }

                        time::sleep(Duration::from_millis(10)).await;
                    }

                    for connections in connector.pool.lock().connections.values() {
                        assert_eq!(connections.len(), 1);
                    }
                }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;
    }

    #[tokio::test]
    async fn one_to_all() {
        let System {
            mut connectors,
            listeners,
            keys,
        } = System::setup(10).await;

        let listeners = listeners
            .into_iter()
            .map(|listener| SessionListener::new(listener))
            .collect::<Vec<_>>();

        tokio::spawn(async move {
            listeners
                .into_iter()
                .map(|mut listener| async move {
                    for _ in 0..10 {
                        let (id, mut session) = listener.accept().await;
                        assert_eq!(session.receive::<u32>().await.unwrap(), 42u32);
                        session.send(&id).await.unwrap();
                        session.end();
                    }
                })
                .collect::<FuturesUnordered<_>>()
                .collect::<Vec<_>>()
                .await;
        });

        let connector = Arc::new(SessionConnector::new(connectors.remove(0)));
        let identity = keys[0].clone();

        keys.into_iter()
            .map(|remote| {
                let connector = connector.clone();
                async move {
                    for _ in 0..10 {
                        {
                            let mut session = connector.connect(remote).await.unwrap();
                            session.send(&42u32).await.unwrap();
                            assert_eq!(session.receive::<Identity>().await.unwrap(), identity);
                            session.end();
                        }
                        time::sleep(Duration::from_millis(10)).await;
                    }
                }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;

        time::sleep(Duration::from_millis(10)).await;

        for connections in connector.pool.lock().connections.values() {
            assert_eq!(connections.len(), 1);
        }
    }

    #[tokio::test]
    async fn all_to_all() {
        let System {
            connectors,
            listeners,
            keys,
        } = System::setup(10).await;

        let connectors = connectors
            .into_iter()
            .map(|connector| Arc::new(SessionConnector::new(connector)))
            .collect::<Vec<_>>();

        let listeners = listeners
            .into_iter()
            .map(|listener| SessionListener::new(listener))
            .collect::<Vec<_>>();

        tokio::spawn(async move {
            listeners
                .into_iter()
                .map(|mut listener| async move {
                    for _ in 0..10 * 10 {
                        let (id, mut session) = listener.accept().await;
                        assert_eq!(session.receive::<u32>().await.unwrap(), 42u32);
                        session.send(&id).await.unwrap();
                        session.end();
                    }
                })
                .collect::<FuturesUnordered<_>>()
                .collect::<Vec<_>>()
                .await;
        });

        connectors
            .into_iter()
            .zip(keys.clone())
            .map(|(connector, identity)| {
                let keys = keys.clone();

                async move {
                    keys.into_iter()
                        .map(|remote| {
                            let connector = connector.clone();
                            async move {
                                for _ in 0..10 {
                                    {
                                        let mut session = connector.connect(remote).await.unwrap();
                                        session.send(&42u32).await.unwrap();
                                        assert_eq!(
                                            session.receive::<Identity>().await.unwrap(),
                                            identity
                                        );
                                        session.end();
                                    }

                                    time::sleep(Duration::from_millis(50)).await;
                                }
                            }
                        })
                        .collect::<FuturesUnordered<_>>()
                        .collect::<Vec<_>>()
                        .await;

                    for connections in connector.pool.lock().connections.values() {
                        assert_eq!(connections.len(), 1);
                    }
                }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;
    }

    #[tokio::test]
    #[ignore]
    async fn keepalive_sequence() {
        let System {
            mut connectors,
            mut listeners,
            keys,
        } = System::setup(2).await;

        let connector = SessionConnector::new(connectors.remove(0));
        let mut listener = SessionListener::new(listeners.remove(1));

        tokio::spawn(async move {
            for _ in 0..3 {
                let (_, mut session) = listener.accept().await;
                assert_eq!(session.receive::<u32>().await.unwrap(), 42u32);
                session.send(&43u32).await.unwrap();
                session.end();
            }
        });

        for _ in 0..3 {
            {
                let mut session = connector.connect(keys[1]).await.unwrap();
                session.send(&42u32).await.unwrap();
                assert_eq!(session.receive::<u32>().await.unwrap(), 43u32);
                session.end();
            }

            time::sleep(Duration::from_secs(100)).await;
        }

        for connections in connector.pool.lock().connections.values() {
            assert_eq!(connections.len(), 1);
        }
    }
}
