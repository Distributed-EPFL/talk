use crate::{
    crypto::Identity,
    net::{
        plex::{Multiplex, MultiplexSettings, Plex, PlexListenerSettings, Role},
        Listener, SecureConnection,
    },
    sync::fuse::Fuse,
};
use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};

type PlexInlet = MpscSender<(Identity, Plex)>;
type PlexOutlet = MpscReceiver<(Identity, Plex)>;

pub struct PlexListener {
    accept_outlet: PlexOutlet,
    _fuse: Fuse,
}

impl PlexListener {
    pub fn new<L>(listener: L, settings: PlexListenerSettings) -> Self
    where
        L: Listener,
    {
        let (accept_inlet, accept_outlet) = mpsc::channel(settings.accept_channel_capacity);

        let fuse = Fuse::new();

        fuse.spawn(PlexListener::listen(listener, accept_inlet, settings));

        PlexListener {
            accept_outlet,
            _fuse: fuse,
        }
    }

    pub async fn accept(&mut self) -> (Identity, Plex) {
        // `accept_inlet` is held by `listen`, whose `Fuse` is
        // held by `self`: the following `recv()` cannot fail
        self.accept_outlet.recv().await.unwrap()
    }

    async fn listen<L>(mut listener: L, accept_inlet: PlexInlet, settings: PlexListenerSettings)
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
                    settings.multiplex_settings.clone(),
                ));
            }
        }
    }

    async fn serve(
        remote: Identity,
        connection: SecureConnection,
        accept_inlet: PlexInlet,
        multiplex_settings: MultiplexSettings,
    ) {
        let multiplex = Multiplex::new(Role::Listener, connection, multiplex_settings);
        let (_, mut listen_multiplex) = multiplex.split();

        while let Ok(plex) = listen_multiplex.accept().await {
            let _ = accept_inlet.send((remote, plex)).await;
        }
    }
}
