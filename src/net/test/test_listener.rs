use async_trait::async_trait;

use crate::{
    crypto::{primitives::sign::PublicKey, KeyChain},
    errors::DynError,
    net::{
        test::errors::test_listener::{ListenError, ListenInterrupted},
        Listener, PlainConnection, SecureConnection,
    },
    sync::fuse::{Fuse, Relay},
};

use snafu::ResultExt;

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
    ) -> Result<(), ListenError> {
        loop {
            if let Ok((stream, _)) = relay
                .map(listener.accept())
                .await
                .context(ListenInterrupted)?
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
    async fn accept(
        &mut self,
    ) -> Result<(PublicKey, SecureConnection), DynError> {
        // `inlet` is dropped only when `fuse` burns: if `outlet.recv()`
        // returned `None`, it would mean that the `Listener` was dropped,
        // which is impossible since `NetListener::accept` is being called
        Ok(self.outlet.recv().await.unwrap())
    }
}
