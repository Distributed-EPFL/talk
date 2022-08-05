use crate::{net::Message, sync::fuse::Fuse};

use rand::prelude::*;

use std::{net::SocketAddr, sync::Arc};

use tokio::sync::mpsc::Sender as MpscSender;

type MessageInlet<M> = MpscSender<(SocketAddr, M)>;

pub struct DatagramSender<S: Message> {
    datagram_inlets: Vec<MessageInlet<S>>,
    _fuse: Arc<Fuse>,
}

impl<S> DatagramSender<S>
where
    S: Message,
{
    pub(in crate::net::datagram_dispatcher) fn new(
        packet_inlets: Vec<MessageInlet<S>>,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramSender {
            datagram_inlets: packet_inlets,
            _fuse: fuse,
        }
    }

    pub async fn send(&self, destination: SocketAddr, message: S) {
        let router = random::<usize>() % self.datagram_inlets.len();

        // Because this `DatagramSender` is holding a copy
        // of the `DatagramDispatcher`'s fuse, the corresponding
        // inlet is guaranteed to still be held by `route_out` tasks
        let _ = self.datagram_inlets[router]
            .send((destination, message))
            .await;
    }
}
