use async_trait::async_trait;

use crate::{
    crypto::{primitives::sign::PublicKey, KeyChain},
    link::rendezvous::{Client, ListenerSettings},
    net::{
        traits::TcpConnect, Listener as NetListener, PlainConnection,
        SecureConnection,
    },
    sync::fuse::{Fuse, Relay},
};

use doomstack::{here, Doom, ResultExt, Stack, Top};

use std::net::Ipv4Addr;

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

type Outlet = Receiver<(PublicKey, SecureConnection)>;

pub struct Listener {
    outlet: Outlet,
    fuse: Fuse,
}

#[derive(Doom)]
enum ListenError {
    #[doom(description("`listen` interrupted"))]
    ListenInterrupted,
}

#[derive(Doom)]
enum ServeError {
    #[doom(description("`serve` interrupted"))]
    ServeInterrupted,
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
            (Ipv4Addr::UNSPECIFIED, 0), // TODO: Determine if `Ipv6Addr` can be used instead (problems with Docker?)
        )
        .await
        .unwrap();

        let root = keychain.keycard().root();
        let port = listener.local_addr().unwrap().port();

        let fuse = Fuse::new();
        let relay = fuse.relay();

        let (inlet, outlet) = mpsc::channel(settings.channel_capacity);

        tokio::spawn(async move {
            let _ = Listener::listen(keychain, listener, inlet, relay).await;
        });

        let client = Client::new(server, settings.client_settings);
        client.advertise_port(root, port).await;

        Listener { outlet, fuse }
    }

    async fn listen(
        keychain: KeyChain,
        listener: TcpListener,
        inlet: Sender<(PublicKey, SecureConnection)>,
        mut relay: Relay,
    ) -> Result<(), Top<ListenError>> {
        let fuse = Fuse::new();

        loop {
            if let Ok((stream, _)) = relay
                .map(listener.accept())
                .await
                .pot(ListenError::ListenInterrupted, here!())?
            {
                let connection = stream.into();

                let keychain = keychain.clone();
                let inlet = inlet.clone();
                let relay = fuse.relay();

                tokio::spawn(async move {
                    let _ = Listener::serve(connection, keychain, inlet, relay)
                        .await;
                });
            }
        }
    }

    async fn serve(
        connection: PlainConnection,
        keychain: KeyChain,
        inlet: Sender<(PublicKey, SecureConnection)>,
        mut relay: Relay,
    ) -> Result<(), Top<ServeError>> {
        let mut connection = relay
            .map(connection.secure())
            .await
            .pot(ServeError::ServeInterrupted, here!())?
            .pot(ServeError::SecureFailed, here!())?;

        let keycard = relay
            .map(connection.authenticate(&keychain))
            .await
            .pot(ServeError::ServeInterrupted, here!())?
            .pot(ServeError::AuthenticateFailed, here!())?;

        // This can only fail if the (local) receiving end is
        // dropped, in which case we don't care about the error
        let _ = inlet.try_send((keycard.root(), connection));

        Ok(())
    }
}

#[async_trait]
impl NetListener for Listener {
    async fn accept(&mut self) -> Result<(PublicKey, SecureConnection), Stack> {
        // `inlet` is dropped only when `fuse` burns: if `outlet.recv()`
        // returned `None`, it would mean that the `Listener` was dropped,
        // which is impossible since `NetListener::accept` is being called
        Ok(self.outlet.recv().await.unwrap())
    }
}
