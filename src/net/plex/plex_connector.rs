use crate::{
    crypto::Identity,
    net::{
        plex::{ConnectMultiplex, Multiplex, MultiplexId, Plex, PlexConnectorSettings, Role},
        Connector as NetConnector,
    },
    sync::fuse::Fuse,
};
use doomstack::{here, Doom, ResultExt, Top};
use parking_lot::Mutex as ParkingMutex;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::Mutex as TokioMutex, time};

pub struct PlexConnector {
    connector: Arc<dyn NetConnector>,
    pool: Arc<ParkingMutex<Pool>>,
    cursor: AtomicUsize,
    settings: PlexConnectorSettings,
    _fuse: Fuse,
}

struct Pool {
    multiplexes: HashMap<Identity, Arc<TokioMutex<HashMap<usize, ConnectMultiplex>>>>,
}

#[derive(Doom)]
pub enum PlexConnectorError {
    #[doom(description("failed when connecting to remote"))]
    ConnectFailed,
}

impl PlexConnector {
    pub fn new<C>(connector: C, settings: PlexConnectorSettings) -> Self
    where
        C: NetConnector,
    {
        let connector = Arc::new(connector);
        let pool = Arc::new(ParkingMutex::new(Pool::new()));
        let cursor = AtomicUsize::new(0);

        let fuse = Fuse::new();

        fuse.spawn(PlexConnector::keep_alive(pool.clone(), settings.clone()));

        PlexConnector {
            connector,
            pool,
            cursor,
            settings,
            _fuse: fuse,
        }
    }

    pub async fn multiplexes_to(&self, remote: Identity) -> Vec<MultiplexId> {
        if let Some(multiplexes) = self.pool.lock().multiplexes.get(&remote) {
            let multiplexes = multiplexes.lock().await;

            multiplexes
                .keys()
                .copied()
                .map(MultiplexId)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    }

    pub async fn fill<R>(&self, remotes: R, interval: Duration)
    where
        R: IntoIterator<Item = Identity>,
    {
        let fuse = Fuse::new();

        let remote_handles = remotes
            .into_iter()
            .map(|remote| {
                let connector = self.connector.clone();
                let connections_per_remote = self.settings.connections_per_remote;

                fuse.spawn(async move {
                    let fuse = Fuse::new();

                    let mut connect_handles = Vec::new();

                    for _ in 0..connections_per_remote {
                        let connector = connector.clone();

                        let connect_handle =
                            fuse.spawn(async move { connector.connect(remote).await });

                        connect_handles.push(connect_handle);
                        time::sleep(interval).await;
                    }

                    let mut connections = Vec::new();

                    for connect_handle in connect_handles {
                        if let Ok(connection) = connect_handle.await.unwrap().unwrap() {
                            connections.push(connection)
                        }
                    }

                    (remote, connections)
                })
            })
            .collect::<Vec<_>>();

        for remote_handle in remote_handles {
            let (remote, connections) = remote_handle.await.unwrap().unwrap();

            let multiplexes = self.pool.lock().get_multiplexes(remote);
            let mut multiplexes = multiplexes.lock().await;

            let missing = self.settings.connections_per_remote - multiplexes.len();

            multiplexes.extend(
                connections
                    .into_iter()
                    .map(|connection| {
                        let multiplex = Multiplex::new(
                            Role::Connector,
                            connection,
                            self.settings.multiplex_settings.clone(),
                        );

                        let (multiplex, _) = multiplex.split();
                        let id = self.cursor.fetch_add(1, Ordering::Relaxed);

                        (id, multiplex)
                    })
                    .take(missing),
            );
        }
    }

    pub async fn connect(&self, remote: Identity) -> Result<Plex, Top<PlexConnectorError>> {
        let multiplexes = self.pool.lock().get_multiplexes(remote);
        let mut multiplexes = multiplexes.lock().await;

        // Prune all dead `ConnectMultiplex`es in `multiplexes`

        multiplexes.retain(|_, multiplex| multiplex.is_alive());

        // Select a `ConnectMultiplex` to `connect` on

        let multiplex = if multiplexes.len() < self.settings.connections_per_remote {
            // More `SecureConnection`s can still be established to `remote`: add
            // a new `ConnectMultiplex` to `multiplexes` and return its reference

            let connection = self
                .connector
                .connect(remote)
                .await
                .pot(PlexConnectorError::ConnectFailed, here!())?;

            let multiplex = Multiplex::new(
                Role::Connector,
                connection,
                self.settings.multiplex_settings.clone(),
            );

            let (multiplex, _) = multiplex.split();
            let id = self.cursor.fetch_add(1, Ordering::Relaxed);

            multiplexes.insert(id, multiplex);
            multiplexes.get_mut(&id).unwrap()
        } else {
            // The target number of `SecureConnection`s has been reached for `remote`:
            // return a reference to the least-loaded `ConnectMultiplex` in `multiplexes`
            // (i.e., the `ConnectMultiplex` managing the least `Plex`es)

            let (_, multiplex) = multiplexes
                .iter_mut()
                .min_by_key(|(_, multiplex)| multiplex.plex_count())
                .unwrap();

            multiplex
        };

        Ok(multiplex.connect().await)
    }

    async fn keep_alive(pool: Arc<ParkingMutex<Pool>>, settings: PlexConnectorSettings) {
        loop {
            {
                let all_multiplexes = pool.lock().all_multiplexes();

                for multiplexes in all_multiplexes {
                    let mut multiplexes = multiplexes.lock().await;

                    // Prune all dead `ConnectMultiplex`es in `multiplexes`

                    multiplexes.retain(|_, multiplex| multiplex.is_alive());

                    // `ping()` all remaining `ConnectMultiplex`es in `multiplexes`

                    for (_, multiplex) in multiplexes.iter() {
                        multiplex.ping();
                    }
                }

                // Drop all remotes with no `ConnectMultiplex`ex

                pool.lock().prune();
            }

            time::sleep(settings.keep_alive_interval).await;
        }
    }
}

impl Pool {
    fn new() -> Self {
        Pool {
            multiplexes: HashMap::new(),
        }
    }

    fn get_multiplexes(
        &mut self,
        remote: Identity,
    ) -> Arc<TokioMutex<HashMap<usize, ConnectMultiplex>>> {
        self.multiplexes.entry(remote).or_default().clone()
    }

    fn all_multiplexes(&mut self) -> Vec<Arc<TokioMutex<HashMap<usize, ConnectMultiplex>>>> {
        self.multiplexes.values().cloned().collect()
    }

    fn prune(&mut self) {
        self.multiplexes.retain(|_, multiplexes| {
            if let Ok(multiplexes) = multiplexes.try_lock() {
                !multiplexes.is_empty()
            } else {
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{plex::PlexListener, test::System};
    use std::time::Duration;

    #[tokio::test]
    async fn single() {
        let System {
            mut connectors,
            mut listeners,
            keys,
        } = System::setup(2).await;

        let connector = PlexConnector::new(connectors.remove(0), Default::default());
        let mut listener = PlexListener::new(listeners.remove(1), Default::default());

        tokio::spawn(async move {
            let (_, mut plex) = listener.accept().await;
            assert_eq!(plex.receive::<u32>().await.unwrap(), 42u32);
            plex.send(&43u32).await.unwrap();
        });

        let mut plex = connector.connect(keys[1]).await.unwrap();
        plex.send(&42u32).await.unwrap();
        assert_eq!(plex.receive::<u32>().await.unwrap(), 43u32);
    }

    #[tokio::test]
    async fn sequence() {
        let System {
            mut connectors,
            mut listeners,
            keys,
        } = System::setup(2).await;

        let connector = PlexConnector::new(
            connectors.remove(0),
            PlexConnectorSettings {
                connections_per_remote: 5,
                ..Default::default()
            },
        );

        let mut listener = PlexListener::new(listeners.remove(1), Default::default());

        tokio::spawn(async move {
            loop {
                let (_, mut plex) = listener.accept().await;
                assert_eq!(plex.receive::<u32>().await.unwrap(), 42u32);
                plex.send(&43u32).await.unwrap();
            }
        });

        for _ in 0..50 {
            let mut plex = connector.connect(keys[1]).await.unwrap();
            plex.send(&42u32).await.unwrap();
            assert_eq!(plex.receive::<u32>().await.unwrap(), 43u32);
        }
    }

    #[tokio::test]
    async fn parallel() {
        let System {
            mut connectors,
            mut listeners,
            keys,
        } = System::setup(2).await;

        let connector = PlexConnector::new(
            connectors.remove(0),
            PlexConnectorSettings {
                connections_per_remote: 5,
                ..Default::default()
            },
        );

        let mut listener = PlexListener::new(listeners.remove(1), Default::default());

        tokio::spawn(async move {
            loop {
                let (_, mut plex) = listener.accept().await;

                tokio::spawn(async move {
                    let value = plex.receive::<u32>().await.unwrap();
                    plex.send(&(value + 1)).await.unwrap();
                });
            }
        });

        let connector = Arc::new(connector);

        let handles = (0..50)
            .map(|value| {
                let connector = connector.clone();
                let remote = keys[1];

                tokio::spawn(async move {
                    let mut plex = connector.connect(remote).await.unwrap();
                    time::sleep(Duration::from_millis(500)).await;
                    plex.send(&value).await.unwrap();
                    assert_eq!(plex.receive::<u32>().await.unwrap(), value + 1);
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn multiple_parallel() {
        let System {
            mut connectors,
            listeners,
            keys,
        } = System::setup(5).await;

        let connector = PlexConnector::new(
            connectors.remove(0),
            PlexConnectorSettings {
                connections_per_remote: 5,
                ..Default::default()
            },
        );

        let listeners = listeners
            .into_iter()
            .skip(1)
            .map(|listener| PlexListener::new(listener, Default::default()))
            .collect::<Vec<_>>();

        for mut listener in listeners {
            tokio::spawn(async move {
                loop {
                    let (_, mut plex) = listener.accept().await;

                    tokio::spawn(async move {
                        let value = plex.receive::<u32>().await.unwrap();
                        plex.send(&(value + 1)).await.unwrap();
                    });
                }
            });
        }

        let connector = Arc::new(connector);

        let handles = (0..200)
            .map(|value| {
                let connector = connector.clone();
                let remote = keys[(value % 4 + 1) as usize];

                tokio::spawn(async move {
                    let mut plex = connector.connect(remote).await.unwrap();
                    time::sleep(Duration::from_millis(500)).await;
                    plex.send(&value).await.unwrap();
                    assert_eq!(plex.receive::<u32>().await.unwrap(), value + 1);
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn receive_drop() {
        let System {
            mut connectors,
            mut listeners,
            keys,
        } = System::setup(2).await;

        let connector = PlexConnector::new(connectors.remove(0), Default::default());
        let mut listener = PlexListener::new(listeners.remove(1), Default::default());

        let handle = tokio::spawn(async move {
            let (_, mut plex) = listener.accept().await;
            plex.receive::<u32>().await.unwrap_err();
        });

        connector.connect(keys[1]).await.unwrap();

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn receive_send() {
        let System {
            mut connectors,
            mut listeners,
            keys,
        } = System::setup(2).await;

        let connector = PlexConnector::new(connectors.remove(0), Default::default());
        let mut listener = PlexListener::new(listeners.remove(1), Default::default());

        tokio::spawn(async move {
            listener.accept().await;
        });

        let mut plex = connector.connect(keys[1]).await.unwrap();

        while plex.send(&42u32).await.is_ok() {
            time::sleep(Duration::from_millis(10)).await;
        }
    }
}
