use crate::{
    crypto::Identity,
    net::{
        plex::{ConnectMultiplex, Multiplex, Plex, PlexConnectorSettings, Role},
        Connector as NetConnector,
    },
    sync::fuse::Fuse,
};
use doomstack::{here, Doom, ResultExt, Top};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc};
use tokio::time;

pub struct PlexConnector {
    connector: Arc<dyn NetConnector>,
    pool: Arc<Mutex<Pool>>,
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
        let connector = Arc::new(connector);

        let pool = Pool {
            multiplexes: HashMap::new(),
        };

        let pool = Arc::new(Mutex::new(pool));

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
        let mut pool = self.pool.lock();
        let multiplexes = pool.multiplexes.entry(remote).or_default();

        PlexConnector::prune(multiplexes);

        let multiplex = if multiplexes.len() < self.settings.connections_per_remote {
            let connection = self
                .connector
                .connect(remote)
                .await
                .pot(PlexConnectorError::ConnectFailed, here!())?;

            let multiplex_settings = self.settings.multiplex_settings.clone();
            let multiplex = Multiplex::new(Role::Connector, connection, multiplex_settings);

            let (multiplex, _) = multiplex.split();

            multiplexes.push(multiplex);
            multiplexes.last_mut().unwrap()
        } else {
            multiplexes
                .iter_mut()
                .min_by_key(|multiplex| multiplex.plex_count())
                .unwrap()
        };

        Ok(multiplex.connect().await)
    }

    async fn keep_alive(pool: Arc<Mutex<Pool>>, settings: PlexConnectorSettings) {
        loop {
            {
                let mut pool = pool.lock();

                pool.multiplexes.retain(|_, multiplexes| {
                    PlexConnector::prune(multiplexes);

                    for multiplex in multiplexes.iter() {
                        multiplex.ping();
                    }

                    !multiplexes.is_empty()
                });
            }

            time::sleep(settings.keep_alive_interval).await;
        }
    }

    fn prune(multiplexes: &mut Vec<ConnectMultiplex>) {
        multiplexes.retain(|multiplex| multiplex.is_alive());
    }
}
