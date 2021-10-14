use async_trait::async_trait;

use crate::{
    crypto::{primitives::sign::PublicKey, KeyChain},
    net::{Listener, PlainConnection, SecureConnection},
    sync::fuse::{Fuse, Relay},
};

use doomstack::{here, Doom, ResultExt, Stack, Top};

use std::{net::Ipv4Addr, net::SocketAddr};

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

const CHANNEL_CAPACITY: usize = 32;

type Outlet = Receiver<(PublicKey, SecureConnection)>;

pub struct TestListener {
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

impl TestListener {
    pub async fn new(keychain: KeyChain) -> (Self, SocketAddr) {
        let listener =
            TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap();

        let address = listener.local_addr().unwrap();

        let fuse = Fuse::new();
        let relay = fuse.relay();

        let (inlet, outlet) = mpsc::channel(CHANNEL_CAPACITY);

        tokio::spawn(async move {
            let _ =
                TestListener::listen(keychain, listener, inlet, relay).await;
        });

        (TestListener { outlet, fuse }, address)
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
                    let _ =
                        TestListener::serve(connection, keychain, inlet, relay).await;
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
        let _ = inlet.send((keycard.root(), connection)).await;

        Ok(())
    }
}

#[async_trait]
impl Listener for TestListener {
    async fn accept(&mut self) -> Result<(PublicKey, SecureConnection), Stack> {
        // `inlet` is dropped only when `fuse` burns: if `outlet.recv()`
        // returned `None`, it would mean that the `Listener` was dropped,
        // which is impossible since `NetListener::accept` is being called
        Ok(self.outlet.recv().await.unwrap())
    }
}
