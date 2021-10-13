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

    // TODO: fix slow-loris
    async fn listen(
        keychain: KeyChain,
        listener: TcpListener,
        inlet: Sender<(PublicKey, SecureConnection)>,
        mut relay: Relay,
    ) -> Result<(), Top<ListenError>> {
        loop {
            if let Ok((stream, _)) = relay
                .map(listener.accept())
                .await
                .pot(ListenError::ListenInterrupted, here!())?
            {
                if let Ok(mut connection) =
                    PlainConnection::from(stream).secure().await
                {
                    if let Ok(keycard) =
                        connection.authenticate(&keychain).await
                    {
                        let _ = inlet.send((keycard.root(), connection)).await;
                    }
                }
            }
        }
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
