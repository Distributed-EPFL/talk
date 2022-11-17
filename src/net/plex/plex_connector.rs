use crate::{
    crypto::Identity,
    net::{
        plex::{ConnectMultiplex, Multiplex, Plex, PlexConnectorSettings, Role},
        Connector as NetConnector,
    },
    sync::fuse::Fuse,
};
use doomstack::{here, Doom, ResultExt, Top};
use std::{collections::HashMap, sync::Arc};
use tokio::{sync::Mutex as TokioMutex, time};

pub struct PlexConnector {
    connector: Box<dyn NetConnector>,
    pool: Arc<TokioMutex<Pool>>,
    settings: PlexConnectorSettings,
    _fuse: Fuse,
}

struct Pool {
    multiplexes: HashMap<Identity, Vec<ConnectMultiplex>>,
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
        let connector = Box::new(connector);

        let pool = Pool {
            multiplexes: HashMap::new(),
        };

        let pool = Arc::new(TokioMutex::new(pool));

        let fuse = Fuse::new();

        fuse.spawn(PlexConnector::keep_alive(pool.clone(), settings.clone()));

        PlexConnector {
            connector,
            pool,
            settings,
            _fuse: fuse,
        }
    }

    pub async fn connect(&self, remote: Identity) -> Result<Plex, Top<PlexConnectorError>> {
        let mut pool = self.pool.lock().await;
        let multiplexes = pool.multiplexes.entry(remote).or_default();

        // Prune all dead `ConnectMultiplex`es in `multiplexes`

        multiplexes.retain(|multiplex| multiplex.is_alive());

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

            multiplexes.push(multiplex);
            multiplexes.last_mut().unwrap()
        } else {
            // The target number of `SecureConnection`s has been reached for `remote`:
            // return a reference to the least-loaded `ConnectMultiplex` in `multiplexes`
            // (i.e., the `ConnectMultiplex` managing the least `Plex`es)

            multiplexes
                .iter_mut()
                .min_by_key(|multiplex| multiplex.plex_count())
                .unwrap()
        };

        Ok(multiplex.connect().await)
    }

    async fn keep_alive(pool: Arc<TokioMutex<Pool>>, settings: PlexConnectorSettings) {
        loop {
            {
                let mut pool = pool.lock().await;

                pool.multiplexes.retain(|_, multiplexes| {
                    // Prune all dead `ConnectMultiplex`es in `multiplexes`

                    multiplexes.retain(|multiplex| multiplex.is_alive());

                    // `ping()` all remaining `ConnectMultiplex`es in `multiplexes`

                    for multiplex in multiplexes.iter() {
                        multiplex.ping();
                    }

                    // If `multiplexes` is empty, drop key and value from `pool.multiplexes`

                    !multiplexes.is_empty()
                });
            }

            time::sleep(settings.keep_alive_interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{plex::PlexListener, test::System};

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
}
