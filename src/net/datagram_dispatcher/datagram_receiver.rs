use crate::{net::Message, sync::fuse::Fuse};

use std::{net::SocketAddr, sync::Arc};

use tokio::sync::mpsc::Receiver as MpscReceiver;

type MessageOutlet<M> = MpscReceiver<(SocketAddr, M)>;

pub struct DatagramReceiver<R: Message> {
    receive_outlet: MessageOutlet<R>,
    _fuse: Arc<Fuse>,
}

impl<R> DatagramReceiver<R>
where
    R: Message,
{
    pub(in crate::net::datagram_dispatcher) fn new(
        receive_outlet: MessageOutlet<R>,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramReceiver {
            receive_outlet,
            _fuse: fuse,
        }
    }

    pub async fn receive(&mut self) -> (SocketAddr, R) {
        self.receive_outlet.recv().await.unwrap()
    }
}
