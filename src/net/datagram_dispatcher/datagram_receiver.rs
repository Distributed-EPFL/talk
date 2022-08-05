use std::{net::SocketAddr, sync::Arc};

use tokio::sync::mpsc::Receiver as MpscReceiver;

use crate::{net::Message, sync::fuse::Fuse};

type MessageOutlet<M> = MpscReceiver<(SocketAddr, M)>;

pub struct DatagramReceiver<R: Message> {
    datagram_outlet: MessageOutlet<R>,
    _fuse: Arc<Fuse>,
}

impl<R> DatagramReceiver<R>
where
    R: Message,
{
    pub(in crate::net::datagram_dispatcher) fn new(
        packet_outlet: MessageOutlet<R>,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramReceiver {
            datagram_outlet: packet_outlet,
            _fuse: fuse,
        }
    }

    pub async fn receive(&mut self) -> (SocketAddr, R) {
        // Because this `DatagramReceiver` is holding a copy
        // of the `DatagramDispatcher`'s fuse, the corresponding
        // inlet is guaranteed to still be held by `process` tasks
        self.datagram_outlet.recv().await.unwrap()
    }
}
