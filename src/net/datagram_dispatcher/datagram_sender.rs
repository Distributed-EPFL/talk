use atomic_counter::AtomicCounter;

use crate::{
    net::{datagram_dispatcher::Statistics, Message},
    sync::fuse::Fuse,
};

use rand::prelude::*;

use std::{net::SocketAddr, sync::Arc, time::Instant};

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

    pub fn retransmission_queue_len(&self) -> usize {
        *self.statistics.retransmission_queue_len.lock().unwrap()
    }

    pub fn next_retransmission(&self) -> Option<Instant> {
        *self.statistics.next_retransmission.lock().unwrap()
    }

    pub fn last_tick(&self) -> Option<Instant> {
        *self.statistics.last_tick.lock().unwrap()
    }

    pub fn waiting_next_task(&self) -> bool {
        *self.statistics.waiting_next_task.lock().unwrap()
    }
}
