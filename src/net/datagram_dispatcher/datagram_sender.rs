use atomic_counter::AtomicCounter;

use crate::{
    net::{
        datagram_dispatcher::{DatagramDispatcherSettings, Statistics},
        Message,
    },
    sync::fuse::Fuse,
};

use rand::prelude::*;

use std::{net::SocketAddr, sync::Arc, time::Instant};

use tokio::{sync::mpsc::Sender as MpscSender, time};

type MessageInlet<M> = MpscSender<(SocketAddr, M)>;

pub struct DatagramSender<S: Message> {
    process_out_inlets: Vec<MessageInlet<S>>,
    settings: DatagramDispatcherSettings,
    statistics: Arc<Statistics>,
    _fuse: Arc<Fuse>,
}

impl<S> DatagramSender<S>
where
    S: Message,
{
    pub(in crate::net::datagram_dispatcher) fn new(
        process_out_inlets: Vec<MessageInlet<S>>,
        settings: DatagramDispatcherSettings,
        statistics: Arc<Statistics>,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramSender {
            process_out_inlets,
            settings,
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

    pub async fn pace<I>(&self, datagrams: I, rate: f64)
    where
        I: IntoIterator<Item = (SocketAddr, S)>,
    {
        let mut datagrams = datagrams.into_iter();

        let start = Instant::now();
        let mut sent = 0;

        loop {
            let target = (rate * start.elapsed().as_secs_f64()) as usize;

            for _ in sent..target {
                let (destination, message) = if let Some(datagram) = datagrams.next() {
                    datagram
                } else {
                    return;
                };

                self.send(destination, message).await;
            }

            sent = target;

            time::sleep(self.settings.pace_interval).await;
        }
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
