use crate::{
    net::{DatagramDispatcherSettings, Message},
    sync::fuse::Fuse,
};

use rand::prelude::*;

use std::{net::SocketAddr, sync::Arc, time::Instant};

use tokio::{sync::mpsc::Sender as MpscSender, time};

type MessageInlet<M> = MpscSender<(SocketAddr, M)>;

pub struct DatagramSender<S: Message> {
    datagram_inlets: Vec<MessageInlet<S>>,
    settings: DatagramDispatcherSettings,
    _fuse: Arc<Fuse>,
}

impl<S> DatagramSender<S>
where
    S: Message,
{
    pub(in crate::net::datagram_dispatcher) fn new(
        datagram_inlets: Vec<MessageInlet<S>>,
        settings: DatagramDispatcherSettings,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramSender {
            datagram_inlets,
            settings,
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
}
