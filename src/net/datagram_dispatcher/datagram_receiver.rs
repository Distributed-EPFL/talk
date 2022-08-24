use atomic_counter::AtomicCounter;

use crate::{
    net::{datagram_dispatcher::Statistics, Message},
    sync::fuse::Fuse,
};

use std::{net::SocketAddr, sync::Arc};

use tokio::sync::mpsc::Receiver as MpscReceiver;

type MessageOutlet<M> = MpscReceiver<(SocketAddr, M)>;

pub struct DatagramReceiver<R: Message> {
    receive_outlet: MessageOutlet<R>,
    statistics: Arc<Statistics>,
    _fuse: Arc<Fuse>,
}

impl<R> DatagramReceiver<R>
where
    R: Message,
{
    pub(in crate::net::datagram_dispatcher) fn new(
        receive_outlet: MessageOutlet<R>,
        statistics: Arc<Statistics>,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramReceiver {
            receive_outlet,
            statistics,
            _fuse: fuse,
        }
    }

    pub async fn receive(&mut self) -> (SocketAddr, R) {
        self.receive_outlet.recv().await.unwrap()
    }

    pub fn retransmissions(&self) -> usize {
        self.statistics.retransmissions.get()
    }

    pub fn process_in_drops(&self) -> usize {
        self.statistics.process_in_drops.get()
    }
}
