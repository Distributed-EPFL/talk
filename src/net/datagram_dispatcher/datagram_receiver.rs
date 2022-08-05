use std::{net::SocketAddr, sync::Arc};

use tokio::sync::mpsc::Receiver as MpscReceiver;

use crate::{net::Message, sync::fuse::Fuse};

type MessageOutlet<M> = MpscReceiver<(SocketAddr, M)>;

pub struct DatagramReceiver<M: Message> {
    datagram_outlet: MessageOutlet<M>,
    _fuse: Arc<Fuse>,
}

impl<M> DatagramReceiver<M>
where
    M: Message,
{
    pub(in crate::net::datagram_dispatcher) fn new(
        packet_outlet: MessageOutlet<M>,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramReceiver {
            datagram_outlet: packet_outlet,
            _fuse: fuse,
        }
    }

    pub async fn receive(&mut self) -> (SocketAddr, M) {
        // Because this `DatagramReceiver` is holding a copy
        // of the `DatagramDispatcher`'s fuse, the corresponding
        // inlet is guaranteed to still be held by `process` tasks
        self.datagram_outlet.recv().await.unwrap()
    }
}
