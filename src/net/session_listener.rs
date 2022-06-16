use crate::{
    crypto::Identity,
    net::{session_control::SessionControl, Listener, SecureConnection, Session},
    sync::fuse::Fuse,
};

use doomstack::{here, Doom, ResultExt, Top};

use std::time::{Duration, Instant};

use tokio::sync::mpsc::{self, Receiver, Sender};

type ConnectionInlet = Sender<(Identity, SecureConnection)>;
type ConnectionOutlet = Receiver<(Identity, SecureConnection)>;

pub struct SessionListener {
    connection_outlet: ConnectionOutlet,
    return_inlet: ConnectionInlet,
    _fuse: Fuse,
}

#[derive(Doom)]
enum PreserveError {
    #[doom(description("Connection error"))]
    ConnectionError,
    #[doom(description("Unused connection timed out"))]
    Timeout,
}

impl SessionListener {
    pub fn new<L>(listener: L) -> Self
    where
        L: Listener,
    {
        let (connection_inlet, connection_outlet) = mpsc::channel(1024); // TODO: Add settings
        let (return_inlet, return_outlet) = mpsc::channel(1024); // TODO: Add settings

        let fuse = Fuse::new();

        {
            let connection_inlet = connection_inlet.clone();

            fuse.spawn(async move {
                SessionListener::listen(listener, connection_inlet).await;
            });
        }

        fuse.spawn(async move {
            SessionListener::handle_returns(return_outlet, connection_inlet).await;
        });

        SessionListener {
            connection_outlet,
            return_inlet,
            _fuse: fuse,
        }
    }

    pub async fn accept(&mut self) -> (Identity, Session) {
        let (remote, connection) = self.connection_outlet.recv().await.unwrap();
        let session = Session::new(remote, connection, self.return_inlet.clone());

        (remote, session)
    }

    async fn listen<L>(mut listener: L, connection_inlet: ConnectionInlet)
    where
        L: Listener,
    {
        loop {
            if let Ok((remote, connection)) = listener.accept().await {
                let _ = connection_inlet.try_send((remote, connection));
            }
        }
    }

    async fn handle_returns(
        mut return_outlet: ConnectionOutlet,
        connection_inlet: ConnectionInlet,
    ) {
        let fuse = Fuse::new();

        loop {
            if let Some((remote, connection)) = return_outlet.recv().await {
                let connection_inlet = connection_inlet.clone();

                fuse.spawn(async move {
                    let _ = SessionListener::preserve(remote, connection, connection_inlet).await;
                });
            }
        }
    }

    async fn preserve(
        remote: Identity,
        mut connection: SecureConnection,
        connection_inlet: ConnectionInlet,
    ) -> Result<(), Top<PreserveError>> {
        let start = Instant::now();

        loop {
            if start.elapsed() > Duration::from_secs(1800) {
                return PreserveError::Timeout.fail().spot(here!());
            }

            let control = connection
                .receive_raw()
                .await
                .pot(PreserveError::ConnectionError, here!())?;

            match control {
                SessionControl::Connect => {
                    let _ = connection_inlet.try_send((remote, connection));
                    return Ok(());
                }
                SessionControl::KeepAlive => {
                    connection
                        .send(&SessionControl::KeepAlive)
                        .await
                        .pot(PreserveError::ConnectionError, here!())?;
                }
            }
        }
    }
}
