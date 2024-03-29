use crate::{
    net::{datagram_dispatcher::Statistics, Message},
    sync::fuse::Fuse,
};
use atomic_counter::AtomicCounter;
use std::{net::SocketAddr, sync::Arc};

type MessageOutlet<M> = flume::Receiver<(SocketAddr, M)>;

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
        self.receive_outlet.recv_async().await.unwrap()
    }

    pub fn packets_sent(&self) -> usize {
        self.statistics.packets_sent.get()
    }

    pub fn packets_received(&self) -> usize {
        self.statistics.packets_received.get()
    }

    pub fn message_packets_processed(&self) -> usize {
        self.statistics.message_packets_processed.get()
    }

    pub fn acknowledgement_packets_processed(&self) -> usize {
        self.statistics.acknowledgement_packets_processed.get()
    }

    pub fn retransmissions(&self) -> usize {
        self.statistics.retransmissions.get()
    }

    pub fn pace_out_chokes(&self) -> usize {
        self.statistics.pace_out_chokes.get()
    }

    pub fn process_in_drops(&self) -> usize {
        self.statistics.process_in_drops.get()
    }

    pub fn route_out_drops(&self) -> usize {
        self.statistics.route_out_drops.get()
    }
}
