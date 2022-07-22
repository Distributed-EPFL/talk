use std::{net::SocketAddr, sync::Arc};

use tokio::sync::mpsc::Receiver as MpscReceiver;

use crate::sync::fuse::Fuse;

type DatagramOutlet = MpscReceiver<(SocketAddr, Vec<u8>)>;

pub struct DatagramReceiver {
    datagram_outlet: DatagramOutlet,
    _fuse: Arc<Fuse>,
}

impl DatagramReceiver {
    pub(in crate::net::datagram_dispatcher) fn new(
        packet_outlet: DatagramOutlet,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramReceiver {
            datagram_outlet: packet_outlet,
            _fuse: fuse,
        }
    }
}
