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

use std::net::Ipv4Addr;

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

pub struct Listener {
    queue: Receiver<(PublicKey, SecureConnection)>,
    fuse: Fuse,
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

        let (sender, receiver) = mpsc::channel(32);

        tokio::spawn(async move {
            Listener::listen(keychain, listener, sender, relay).await;
        });

        let client = Client::new(server, settings.client_settings);
        client.advertise_port(root, port).await;

        Listener {
            queue: receiver,
            fuse,
        }
    }

    async fn listen(
        keychain: KeyChain,
        listener: TcpListener,
        sender: Sender<(PublicKey, SecureConnection)>,
        mut relay: Relay,
    ) {
        loop {
            let (stream, _) =
                relay.map(listener.accept()).await.unwrap().unwrap();

            let mut connection =
                PlainConnection::from(stream).secure().await.unwrap();

            let keycard = connection.authenticate(&keychain).await.unwrap();

            let _ = sender.send((keycard.root(), connection)).await;
        }
    }
}

#[async_trait]
impl NetListener for Listener {
    type Error = ();

    async fn accept(&mut self) -> Result<(PublicKey, SecureConnection), ()> {
        Ok(self.queue.recv().await.unwrap())
    }
}
