use crate::sync::fuse::Fuse;

use rand::prelude::*;

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

    pub async fn send(&self, destination: SocketAddr, message: Vec<u8>) {
        let router = random::<usize>() % self.datagram_inlets.len();

        // Because this `DatagramSender` is holding a copy
        // of the `DatagramDispatcher`'s fuse, the corresponding
        // inlet is guaranteed to still be held by `route_out` tasks
        self.datagram_inlets[router]
            .send((destination, message))
            .await
            .unwrap();
    }
}
