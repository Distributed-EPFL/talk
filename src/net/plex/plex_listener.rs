use crate::{
    crypto::Identity,
    net::{
        plex::{Multiplex, Plex, Role},
        Listener, SecureConnection,
    },
    sync::fuse::Fuse,
};
use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};

type PlexInlet = MpscSender<(Identity, Plex)>;
type PlexOutlet = MpscReceiver<(Identity, Plex)>;

// TODO: Refactor following constants into settings
const ACCEPT_PLEX_CHANNEL_CAPACITY: usize = 1024;

pub struct PlexListener {
    accept_outlet: PlexOutlet,
    _fuse: Fuse,
}

impl PlexListener {
    pub fn new<L>(listener: L) -> Self
    where
        L: Listener,
    {
        let (accept_inlet, accept_outlet) = mpsc::channel(ACCEPT_PLEX_CHANNEL_CAPACITY);

        let fuse = Fuse::new();

        fuse.spawn(PlexListener::listen(listener, accept_inlet));

        PlexListener {
            accept_outlet,
            _fuse: fuse,
        }
    }

    pub async fn accept(&mut self) -> (Identity, Plex) {
        self.accept_outlet.recv().await.unwrap()
    }

    async fn listen<L>(mut listener: L, accept_inlet: PlexInlet)
    where
        L: Listener,
    {
        let fuse = Fuse::new();

        loop {
            if let Ok((remote, connection)) = listener.accept().await {
                fuse.spawn(PlexListener::serve(
                    remote,
                    connection,
                    accept_inlet.clone(),
                ));
            }
        }
    }

    async fn serve(remote: Identity, connection: SecureConnection, accept_inlet: PlexInlet) {
        let (_, mut plex_listener) =
            Multiplex::new(Role::Listener, connection, Default::default()).split(); // TODO: Replace `Default::default()`

        while let Ok(plex) = plex_listener.accept().await {
            let _ = accept_inlet.send((remote, plex)).await;
        }
    }
}
