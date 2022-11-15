use crate::{
    net::{plex::Payload, SecureConnection, SecureReceiver, SecureSender},
    sync::fuse::Fuse,
};
use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};

type PayloadInlet = MpscSender<Payload>;
type PayloadOutlet = MpscReceiver<Payload>;

// TODO: Refactor following constants into settings
const RUN_SEND_CHANNEL_CAPACITY: usize = 128;
const RUN_ROUTE_IN_CHANNEL_CAPACITY: usize = 128;
const ROUTE_OUT_CHANNEL_CAPACITY: usize = 128;

pub(in crate::net::plex) struct ConnectMultiplex {
    run_send_inlet: PayloadInlet,
    _fuse: Fuse,
}

impl ConnectMultiplex {
    pub fn new(connection: SecureConnection) -> Self {
        let (run_send_inlet, run_send_outlet) = mpsc::channel(RUN_SEND_CHANNEL_CAPACITY);
        let fuse = Fuse::new();

        fuse.spawn(ConnectMultiplex::run(connection, run_send_outlet));

        ConnectMultiplex {
            run_send_inlet,
            _fuse: fuse,
        }
    }

    async fn run(connection: SecureConnection, run_send_outlet: PayloadOutlet) {
        let (sender, receiver) = connection.split();

        let (route_out_inlet, route_out_outlet) = mpsc::channel(ROUTE_OUT_CHANNEL_CAPACITY);

        let (run_route_in_inlet, run_route_in_outlet) =
            mpsc::channel(RUN_ROUTE_IN_CHANNEL_CAPACITY);

        let fuse = Fuse::new();

        fuse.spawn(ConnectMultiplex::route_in(receiver, run_route_in_inlet));
        fuse.spawn(ConnectMultiplex::route_out(sender, route_out_outlet));
    }

    async fn route_out(sender: SecureSender, route_out_outlet: PayloadOutlet) {}

    async fn route_in(receiver: SecureReceiver, run_route_in_inlet: PayloadInlet) {}
}
