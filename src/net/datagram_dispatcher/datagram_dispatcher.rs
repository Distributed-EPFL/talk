use crate::{
    net::datagram_dispatcher::{
        DatagramDispatcherSettings, DatagramReceiver, DatagramSender, ReceiveMultiple, UdpWrap,
    },
    sync::fuse::Fuse,
};

use doomstack::{here, Doom, ResultExt, Top};

use std::{
    collections::{HashMap, VecDeque},
    mem,
    net::SocketAddr,
    sync::Arc,
};

use tokio::{
    net::ToSocketAddrs,
    sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender},
    time,
    time::Instant,
};

type AcknowledgementsInlet = MpscSender<Vec<u64>>;
type AcknowledgementsOutlet = MpscReceiver<Vec<u64>>;

type DatagramInlet = MpscSender<(SocketAddr, Vec<u8>)>;
type DatagramOutlet = MpscReceiver<(SocketAddr, Vec<u8>)>;

type DatagramsInlet = MpscSender<ReceiveMultiple>;
type DatagramsOutlet = MpscReceiver<ReceiveMultiple>;

pub struct DatagramDispatcher {
    sender: DatagramSender,
    receiver: DatagramReceiver,
}

#[derive(Doom)]
pub enum DatagramDispatcherError {
    #[doom(description("Failed to `bind` to the address provided"))]
    BindFailed,
}

impl DatagramDispatcher {
    pub async fn bind<A>(
        address: A,
        settings: DatagramDispatcherSettings,
    ) -> Result<DatagramDispatcher, Top<DatagramDispatcherError>>
    where
        A: Clone + ToSocketAddrs,
    {
        // Wraps

        let mut wraps = Vec::new();

        for _ in 0..settings.workers {
            let wrap = UdpWrap::bind(address.clone(), Default::default())
                .await
                .pot(DatagramDispatcherError::BindFailed, here!())?;

            let wrap = Arc::new(wrap);
            wraps.push(wrap);
        }

        // Receiver inlet

        let (receiver_inlet, receiver_outlet) = mpsc::channel(settings.receiver_channel_capacity);

        // `route_out` task channels

        let mut route_out_inlets = Vec::new();
        let mut route_out_outlets = Vec::new();

        for _ in 0..settings.workers {
            let (route_in_inlet, route_out_outlet) =
                mpsc::channel(settings.route_out_channels_capacity);

            route_out_inlets.push(route_in_inlet);
            route_out_outlets.push(route_out_outlet);
        }

        // Acknowledgement channels

        let mut acknowledgements_inlets = Vec::new();
        let mut acknowledgements_outlets = Vec::new();

        for _ in 0..settings.workers {
            let (acknowledgements_inlet, acknowledgements_outlet) =
                mpsc::channel(settings.acknowledgements_channels_capacity);

            acknowledgements_inlets.push(acknowledgements_inlet);
            acknowledgements_outlets.push(acknowledgements_outlet);
        }

        // `processor` task channels

        let mut process_inlets = Vec::new();
        let mut process_outlets = Vec::new();

        for _ in 0..settings.workers {
            let (process_inlet, process_outlet) = mpsc::channel(settings.process_channels_capacity);

            process_inlets.push(process_inlet);
            process_outlets.push(process_outlet);
        }

        // Tasks

        let fuse = Arc::new(Fuse::new());

        for (index, ((wrap, route_out_outlet), acknowledgements_outlet)) in wraps
            .iter()
            .zip(route_out_outlets)
            .zip(acknowledgements_outlets)
            .enumerate()
        {
            let wrap = wrap.clone();
            let settings = settings.clone();

            fuse.spawn(async move {
                DatagramDispatcher::route_out(
                    index as u8,
                    wrap,
                    route_out_outlet,
                    acknowledgements_outlet,
                    settings,
                )
                .await;
            });
        }

        for (wrap, process_inlet) in wraps.iter().zip(process_inlets) {
            let wrap = wrap.clone();

            fuse.spawn(async move {
                DatagramDispatcher::route_in(wrap, process_inlet).await;
            });
        }

        for (wrap, process_outlet) in wraps.into_iter().zip(process_outlets) {
            let receiver_inlet = receiver_inlet.clone();
            let acknowledgements_inlets = acknowledgements_inlets.clone();
            let settings = settings.clone();

            fuse.spawn(async move {
                DatagramDispatcher::process(
                    wrap,
                    process_outlet,
                    receiver_inlet,
                    acknowledgements_inlets,
                    settings,
                )
                .await;
            });
        }

        // Sender and receiver

        let sender = DatagramSender::new(route_out_inlets, fuse.clone());
        let receiver = DatagramReceiver::new(receiver_outlet, fuse);

        Ok(DatagramDispatcher { sender, receiver })
    }

    pub async fn send(&self, destination: SocketAddr, message: Vec<u8>) {
        self.sender.send(destination, message).await
    }

    pub async fn receive(&mut self) -> (SocketAddr, Vec<u8>) {
        self.receiver.receive().await
    }

    pub fn split(self) -> (DatagramSender, DatagramReceiver) {
        (self.sender, self.receiver)
    }

    async fn route_out(
        index: u8,
        wrap: Arc<UdpWrap>,
        mut route_out_outlet: DatagramOutlet,
        mut acknowledgements_outlet: AcknowledgementsOutlet,
        settings: DatagramDispatcherSettings,
    ) {
        // Data structures
        let mut sequence: u64 = 0;
        let mut datagrams: HashMap<u64, (SocketAddr, Vec<u8>)> =
            HashMap::with_capacity(settings.route_out_channels_capacity);
        let mut retransmissions: VecDeque<(Instant, u64)> =
            VecDeque::with_capacity(settings.route_out_channels_capacity);

        // Buffers
        let mut route_out_buffer: Vec<(SocketAddr, Vec<u8>)> =
            Vec::with_capacity(settings.route_out_channels_capacity);
        let mut acknowledgements_buffer: Vec<u64> =
            Vec::with_capacity(settings.route_out_channels_capacity);
        let mut transmission_buffer: Vec<(u64, (SocketAddr, Vec<u8>))> =
            Vec::with_capacity(settings.route_out_channels_capacity);

        let mut last_retransmission: Instant = Instant::now();

        loop {
            // Invariant: here all buffers are empty: `route_out_buffer`,
            // `acknowledgements_buffer` and `transmission_buffer` are
            // initially empty and `drain`ed before the `loop` is completed.

            // Wait to be woken up

            tokio::select! {
                datagram = route_out_outlet.recv() => {
                    if let Some(datagram) = datagram {
                        route_out_buffer.push(datagram);
                    }
                }
                acknowledgements = acknowledgements_outlet.recv() => {
                    if let Some(mut acknowledgements) = acknowledgements {
                        acknowledgements_buffer.append(&mut acknowledgements);
                    }
                }
                _ = time::sleep_until(last_retransmission + settings.retransmission_interval), if !retransmissions.is_empty() => {}
            }

            // Flush outlets
            while let Ok(mut acknowledgements) = acknowledgements_outlet.try_recv() {
                acknowledgements_buffer.append(&mut acknowledgements);
            }

            // Remove acknowledged `datagrams`

            for acknowledgement in acknowledgements_buffer.drain(..) {
                datagrams.remove(&acknowledgement);
            }

            // Stage `retransmissions` for transmission

            let now = Instant::now();
            let mut has_pending_retransmissions = false;

            if now > last_retransmission + settings.retransmission_interval {
                loop {
                    if let Some((time, _)) = retransmissions.front() {
                        if *time > now {
                            break;
                        }
                        if transmission_buffer.len() >= settings.retransmission_batch_size {
                            has_pending_retransmissions = true;
                            break;
                        }
                    } else {
                        break;
                    }

                    let (_, sequence) = retransmissions.pop_front().unwrap();

                    if let Some(datagram) = datagrams.remove(&sequence) {
                        transmission_buffer.push((sequence, datagram));
                    }
                }
            }

            if !transmission_buffer.is_empty() {
                last_retransmission = now;
            }

            if !has_pending_retransmissions {
                while let Ok(datagram) = route_out_outlet.try_recv() {
                    route_out_buffer.push(datagram);
                }
            }

            // Stage `route_out_buffer` for transmission
            for (address, mut buffer) in route_out_buffer.drain(..) {
                let mut footer = [0u8; 10];
                footer[0] = 0; // Datagram is a message
                footer[1] = index; // Datagram was sent by this router
                footer[2..10].clone_from_slice(&sequence.to_le_bytes()[..]);

                buffer.extend_from_slice(&footer[..]);
                transmission_buffer.push((sequence, (address, buffer)));

                sequence += 1;
            }

            // Invoke `send_multiple`

            let to_send = transmission_buffer.len();
            let send_multiple_iterator = |start| {
                transmission_buffer[start..]
                    .iter()
                    .map(|(_, (address, buffer))| (address, buffer.as_slice()))
            };

            let mut sent = 0;
            while sent < to_send {
                sent += wrap
                    .send_multiple(send_multiple_iterator(sent))
                    .await
                    .expect("send_multiple failed");
            }

            // Populate `datagrams` and `retransmissions`

            let retransmission_time = Instant::now() + settings.retransmission_delay;

            for (sequence, datagram) in transmission_buffer.drain(..) {
                datagrams.insert(sequence, datagram);
                retransmissions.push_back((retransmission_time, sequence));
            }
        }
    }

    async fn route_in(wrap: Arc<UdpWrap>, processor_inlet: DatagramsInlet) {
        loop {
            let datagrams = wrap.receive_multiple().await;
            let _ = processor_inlet.send(datagrams).await;
        }
    }

    async fn process(
        wrap: Arc<UdpWrap>,
        mut process_outlet: DatagramsOutlet,
        receiver_inlet: DatagramInlet,
        acknowledgements_inlets: Vec<AcknowledgementsInlet>,
        settings: DatagramDispatcherSettings,
    ) {
        let mut acknowledgements_out: Vec<(SocketAddr, [u8; 10])> =
            Vec::with_capacity(settings.process_channels_capacity);
        let mut acknowledgements_in: Vec<Vec<u64>> =
            vec![
                Vec::with_capacity(settings.process_channels_capacity);
                acknowledgements_inlets.len()
            ];

        loop {
            let datagrams = match process_outlet.recv().await {
                Some(datagrams) => datagrams,
                None => continue,
            };

            acknowledgements_out.clear();

            for (source, bytes) in datagrams.iter() {
                if bytes.len() < 10 {
                    continue;
                }

                let (message, footer) = bytes.split_at(bytes.len() - 10);

                if footer[0] == 0 {
                    // Datagram is a message

                    // Stage acknowledgement for transmission

                    let mut acknowledgement = [0u8; 10];
                    acknowledgement[..].clone_from_slice(footer);
                    acknowledgement[0] = 1; // Sending back an acknowledgement

                    acknowledgements_out.push((*source, acknowledgement));

                    // Dispatch message to `DatagramReceiver`

                    let _ = receiver_inlet.send(((*source), message.to_vec())).await;
                } else if footer[0] == 1 {
                    // Datagram is an acknowledgement

                    let router = footer[1];

                    let mut sequence = [0u8; 8];
                    sequence[..].clone_from_slice(&footer[2..10]);
                    let sequence = u64::from_le_bytes(sequence);

                    acknowledgements_in[router as usize].push(sequence);
                }
            }

            // Invoke `send_multiple`
            let to_send = acknowledgements_out.len();
            let send_multiple_iterator = |start| {
                acknowledgements_out[start..]
                    .iter()
                    .map(|(address, buffer)| (address, buffer.as_slice()))
            };

            let mut sent = 0;
            while sent < to_send {
                sent += wrap
                    .send_multiple(send_multiple_iterator(sent))
                    .await
                    .expect("send_multiple failed");
            }

            // Dispatch acknowledgements to routers

            for (inlet, acknowledgements) in acknowledgements_inlets
                .iter()
                .zip(acknowledgements_in.iter_mut())
            {
                if !acknowledgements.is_empty() {
                    let acknowledgements = mem::take(acknowledgements);
                    let _ = inlet.send(acknowledgements).await;
                }
            }
        }
    }
}
