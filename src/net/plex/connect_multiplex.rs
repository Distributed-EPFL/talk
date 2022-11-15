use crate::{
    net::{plex::Payload, SecureConnection},
    sync::fuse::Fuse,
};

use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};

type PayloadInlet = MpscSender<Payload>;
type PayloadOutlet = MpscReceiver<Payload>;

// TODO: Refactor following constants into settings
const RUN_CHANNEL_CAPACITY: usize = 128;

pub(in crate::net::plex) struct ConnectMultiplex {
    run_inlet: PayloadInlet,
    _fuse: Fuse,
}

impl ConnectMultiplex {
    pub fn new(connection: SecureConnection) -> Self {
        let (run_inlet, run_outlet) = mpsc::channel(RUN_CHANNEL_CAPACITY);
        let fuse = Fuse::new();

        fuse.spawn(ConnectMultiplex::run(connection, run_outlet));

        ConnectMultiplex {
            run_inlet,
            _fuse: fuse,
        }
    }

    async fn run(connection: SecureConnection, run_outlet: PayloadOutlet) {}
}
