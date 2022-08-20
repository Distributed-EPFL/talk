use crate::{net::Message, sync::fuse::Fuse};

use rand::prelude::*;

use std::{net::SocketAddr, sync::Arc};

use tokio::sync::mpsc::Sender as MpscSender;

type MessageInlet<M> = MpscSender<(SocketAddr, M)>;

pub struct DatagramSender<S: Message> {
    process_out_inlets: Vec<MessageInlet<S>>,
    _fuse: Arc<Fuse>,
}

impl<S> DatagramSender<S>
where
    S: Message,
{
    pub(in crate::net::datagram_dispatcher) fn new(
        process_out_inlets: Vec<MessageInlet<S>>,
        fuse: Arc<Fuse>,
    ) -> Self {
        DatagramSender {
            process_out_inlets,
            _fuse: fuse,
        }
    }

    pub async fn send(&self, destination: SocketAddr, payload: S) {
        let _ = self
            .process_out_inlets
            .choose(&mut thread_rng())
            .unwrap()
            .send((destination, payload))
            .await;
    }
}
