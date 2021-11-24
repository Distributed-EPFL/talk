use async_trait::async_trait;

use crate::{
    crypto::{Identity, KeyChain},
    net::{Listener, PlainConnection, SecureConnection},
    sync::fuse::Fuse,
};

use doomstack::{here, Doom, ResultExt, Stack, Top};

use std::net::{Ipv4Addr, SocketAddr};

use tokio::{
    net::TcpListener,
    sync::{
        mpsc,
        mpsc::{Receiver, Sender},
    },
};

const CHANNEL_CAPACITY: usize = 32;

type Outlet = Receiver<(Identity, SecureConnection)>;

pub struct TestListener {
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

impl TestListener {
    pub async fn new(keychain: KeyChain) -> (Self, SocketAddr) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();

        let address = listener.local_addr().unwrap();

        let fuse = Fuse::new();

        let (inlet, outlet) = mpsc::channel(CHANNEL_CAPACITY);

        fuse.spawn(async move {
            let _ = TestListener::listen(keychain, listener, inlet).await;
        });

        (
            TestListener {
                outlet,
                _fuse: fuse,
            },
            address,
        )
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
                    let _ = TestListener::serve(connection, keychain, inlet).await;
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
impl Listener for TestListener {
    async fn accept(&mut self) -> Result<(Identity, SecureConnection), Stack> {
        // `inlet` is dropped only when `fuse` burns: if `outlet.recv()`
        // returned `None`, it would mean that the `Listener` was dropped,
        // which is impossible since `NetListener::accept` is being called
        Ok(self.outlet.recv().await.unwrap())
    }
}
