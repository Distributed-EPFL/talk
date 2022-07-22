use crate::sync::fuse::Fuse;

use std::{net::SocketAddr, sync::Arc};

use tokio::sync::mpsc::Sender as MpscSender;

type DatagramInlet = MpscSender<(SocketAddr, Vec<u8>)>;

pub struct DatagramSender {
    datagram_inlets: Vec<DatagramInlet>,
    _fuse: Arc<Fuse>,
}

impl DatagramSender {
    pub(in crate::net::datagram_dispatcher) fn new(
        packet_inlets: Vec<DatagramInlet>,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramSender {
            datagram_inlets: packet_inlets,
            _fuse: fuse,
        }
    }
}
