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
impl NetListener for Listener {
    async fn accept(&mut self) -> Result<(PublicKey, SecureConnection), Stack> {
        // `inlet` is dropped only when `fuse` burns: if `outlet.recv()`
        // returned `None`, it would mean that the `Listener` was dropped,
        // which is impossible since `NetListener::accept` is being called
        Ok(self.outlet.recv().await.unwrap())
    }
}
