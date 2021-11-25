use crate::{
    crypto::Identity,
    net::{Connector as NetConnector, SecureConnection, Session, SessionControl},
    sync::{fuse::Fuse, lenders::AtomicLender},
};

use doomstack::{here, Doom, ResultExt, Stack, Top};

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    time,
};

type ReturnInlet = Sender<(Identity, SecureConnection)>;
type ReturnOutlet = Receiver<(Identity, SecureConnection)>;

pub struct SessionConnector {
    connector: Arc<dyn NetConnector>,
    pool: Arc<Mutex<Pool>>,
    return_inlet: ReturnInlet,
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

        let connection = if let Some(states) = pool.connections.get_mut(&remote) {
            let mut restore = Vec::new();

            let connection = (|| {
                while let Some(state) = states.pop() {
                    if let Some(state) = state.try_take() {
                        match state {
                            State::Healthy(connection) => {
                                return Some(connection);
                            }
                            State::Broken => {}
                        }
                    } else {
                        restore.push(state);
                    }
                }

                None
            })();

            states.extend(restore);
            connection
        } else {
            None
        };

        let connection = if let Some(connection) = connection {
            connection
        } else {
            self.connector.connect(remote).await?
        };

        Ok(Session::new(remote, connection, self.return_inlet.clone()))
    }

    async fn handle_returns(pool: Arc<Mutex<Pool>>, mut return_outlet: ReturnOutlet) {
        let fuse = Fuse::new();

        loop {
            if let Some((remote, connection)) = return_outlet.recv().await {
                let connection = Arc::new(AtomicLender::new(State::Healthy(connection)));

                {
                    let connection = connection.clone();
                    let mut pool = pool.lock().unwrap();

                    pool.connections
                        .entry(remote)
                        .or_insert(Vec::new())
                        .push(connection);
                }

                fuse.spawn(async move {
                    let _ = SessionConnector::keep_alive(connection).await;
                });
            }
        }
    }

    async fn keep_alive(state: Arc<AtomicLender<State>>) -> Result<(), Top<KeepAliveError>> {
        loop {
            time::sleep(Duration::from_secs(10)).await; // TODO: Add settings

            let mut connection = if let Some(state) = state.try_take() {
                match state {
                    State::Healthy(connection) => connection,
                    State::Broken => unreachable!(),
                }
            } else {
                return Ok(());
            };

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
