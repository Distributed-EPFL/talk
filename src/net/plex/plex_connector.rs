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
use std::{collections::HashMap, mem, sync::Arc};
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

        let pool = Arc::new(Mutex::new(Pool {
            multiplexes: HashMap::new(),
        }));

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

        if multiplexes
            .iter()
            .filter(|multiplex| multiplex.is_alive())
            .count()
            < self.settings.connections_per_remote
        {
            let connection = self
                .connector
                .connect(remote)
                .await
                .pot(PlexConnectorError::ConnectFailed, here!())?;

            let multiplex_settings = self.settings.multiplex_settings.clone();

            let (mut connect_multiplex, _) =
                Multiplex::new(Role::Connector, connection, multiplex_settings).split();

            let plex = connect_multiplex.connect().await;

            multiplexes.push(connect_multiplex);

            Ok(plex)
        } else {
            let connect_multiplex = multiplexes
                .iter_mut()
                .filter(|multiplex| multiplex.is_alive())
                .min_by_key(|multiplex| multiplex.plex_count())
                .unwrap();

            let plex = connect_multiplex.connect().await;

            Ok(plex)
        }
    }

    async fn keep_alive(pool: Arc<Mutex<Pool>>, settings: PlexConnectorSettings) {
        loop {
            {
                let mut pool = pool.lock();

                for multiplexes in pool.multiplexes.values_mut() {
                    let mut swap = Vec::with_capacity(multiplexes.len());
                    mem::swap(multiplexes, &mut swap);

                    multiplexes.extend(swap.into_iter().filter(|multiplex| {
                        multiplex.ping();
                        multiplex.is_alive()
                    }));
                }
            }

            time::sleep(settings.keep_alive_interval).await;
        }
    }
}
