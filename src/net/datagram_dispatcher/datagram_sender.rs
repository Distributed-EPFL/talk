use atomic_counter::AtomicCounter;

use crate::{
    net::{datagram_dispatcher::Statistics, Message},
    sync::fuse::Fuse,
};

use rand::prelude::*;

use std::{net::SocketAddr, sync::Arc};

use tokio::sync::mpsc::Sender as MpscSender;

type MessageInlet<M> = MpscSender<(SocketAddr, M)>;

pub struct DatagramSender<S: Message> {
    process_out_inlets: Vec<MessageInlet<S>>,
    statistics: Arc<Statistics>,
    _fuse: Arc<Fuse>,
}

impl<S> DatagramSender<S>
where
    S: Message,
{
    pub(in crate::net::datagram_dispatcher) fn new(
        process_out_inlets: Vec<MessageInlet<S>>,
        statistics: Arc<Statistics>,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramSender {
            process_out_inlets,
            statistics,
            _fuse: fuse,
        }
    }

    pub async fn send(&self, destination: SocketAddr, payload: S) {
        let inlet = random::<usize>() % self.process_out_inlets.len();

        let _ = self
            .process_out_inlets
            .get(inlet)
            .unwrap()
            .send((destination, payload))
            .await;
    }

    pub fn retransmissions(&self) -> usize {
        self.statistics.retransmissions.get()
    }
}
