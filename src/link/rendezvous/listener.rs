use async_trait::async_trait;

use crate::{
    crypto::{Identity, KeyChain},
    link::rendezvous::{Client, ListenerSettings},
    net::{
        traits::TcpConnect, Listener as NetListener, PlainConnection,
        SecureConnection,
    },
    sync::fuse::Fuse,
};

use doomstack::{here, Doom, ResultExt, Stack, Top};

use std::net::Ipv4Addr;

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

type Outlet = Receiver<(Identity, SecureConnection)>;

pub struct Listener {
    outlet: Outlet,
    _fuse: Fuse,
}

#[derive(Doom)]
enum ServeError {
    #[doom(description("Failed to `secure` the connection"))]
    SecureFailed,
    #[doom(description("Failed to `authenticate` the connection"))]
    AuthenticateFailed,
}

impl Listener {
    pub async fn new<S>(
        server: S,
        keychain: KeyChain,
        settings: ListenerSettings,
    ) -> Self
    where
        S: 'static + TcpConnect,
    {
        let listener = TcpListener::bind(
            (Ipv4Addr::LOCALHOST, 0), // TODO: Determine if `Ipv6Addr` can be used instead (problems with Docker?)
        )
        .await
        .unwrap();

        let identity = keychain.keycard().identity();
        let port = listener.local_addr().unwrap().port();

        let fuse = Fuse::new();

        let (inlet, outlet) = mpsc::channel(settings.channel_capacity);

        fuse.spawn(async move {
            let _ = Listener::listen(keychain, listener, inlet).await;
        });

        let client = Client::new(server, settings.client_settings);
        client.advertise_port(identity, port).await;

        Listener {
            outlet,
            _fuse: fuse,
        }
    }

    async fn listen(
        keychain: KeyChain,
        listener: TcpListener,
        inlet: Sender<(Identity, SecureConnection)>,
    ) {
        let fuse = Fuse::new();

        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let connection = stream.into();

                let keychain = keychain.clone();
                let inlet = inlet.clone();

                fuse.spawn(async move {
                    let _ = Listener::serve(connection, keychain, inlet).await;
                });
            }
        }
    }

    async fn serve(
        connection: PlainConnection,
        keychain: KeyChain,
        inlet: Sender<(Identity, SecureConnection)>,
    ) -> Result<(), Top<ServeError>> {
        let mut connection = connection
            .secure()
            .await
            .pot(ServeError::SecureFailed, here!())?;

        let keycard = connection
            .authenticate(&keychain)
            .await
            .pot(ServeError::AuthenticateFailed, here!())?;

        // This can only fail if the (local) receiving end is
        // dropped, in which case we don't care about the error
        let _ = inlet.try_send((keycard.identity(), connection));

        Ok(())
    }
}

#[async_trait]
impl NetListener for Listener {
    async fn accept(&mut self) -> Result<(Identity, SecureConnection), Stack> {
        // `inlet` is dropped only when `fuse` burns: if `outlet.recv()`
        // returned `None`, it would mean that the `Listener` was dropped,
        // which is impossible since `NetListener::accept` is being called
        Ok(self.outlet.recv().await.unwrap())
    }
}
