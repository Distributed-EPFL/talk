use crate::{
    crypto::Identity,
    net::{Connector as NetConnector, SecureConnection, Session, SessionControl},
    sync::{fuse::Fuse, lenders::AtomicLender},
};

use doomstack::{here, Doom, ResultExt, Stack, Top};

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
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
        let mut pool = self.pool.lock().unwrap();

        // Try to get a `Healthy` connection from `pool`
        let connection = pool
            .connections
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
            .flatten();

        let connection = if let Some(mut connection) = connection {
            connection.send(&SessionControl::Connect).await?;
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
                    let mut pool = pool.lock().unwrap();

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
                    .send_plain(&SessionControl::KeepAlive)
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
