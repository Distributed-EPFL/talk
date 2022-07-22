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
    time::{Duration, Instant},
};

use tokio::{
    net::ToSocketAddrs,
    sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender},
    time,
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

        let (receiver_inlet, receiver_outlet) = mpsc::channel(1024); // TODO: Add settings

        // `route_out` task channels

        let mut route_out_inlets = Vec::new();
        let mut route_out_outlets = Vec::new();

        for _ in 0..settings.workers {
            let (route_in_inlet, route_out_outlet) = mpsc::channel(1024); // TODO: Add settings

            route_out_inlets.push(route_in_inlet);
            route_out_outlets.push(route_out_outlet);
        }

        // Acknowledgement channels

        let mut acknowledgements_inlets = Vec::new();
        let mut acknowledgements_outlets = Vec::new();

        for _ in 0..settings.workers {
            let (acknowledgements_inlet, acknowledgements_outlet) = mpsc::channel(1024); // TODO: Add settings

            acknowledgements_inlets.push(acknowledgements_inlet);
            acknowledgements_outlets.push(acknowledgements_outlet);
        }

        // `processor` task channels

        let mut process_inlets = Vec::new();
        let mut process_outlets = Vec::new();

        for _ in 0..settings.workers {
            let (process_inlet, process_outlet) = mpsc::channel(1024); // TODO: Add settings

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

            fuse.spawn(async move {
                let _ = DatagramDispatcher::route_out(
                    index as u8,
                    wrap,
                    route_out_outlet,
                    acknowledgements_outlet,
                )
                .await;
            });
        }

        for (wrap, process_inlet) in wraps.iter().zip(process_inlets) {
            let wrap = wrap.clone();

            fuse.spawn(async move {
                let _ = DatagramDispatcher::route_in(wrap, process_inlet).await;
            });
        }

        for (wrap, process_outlet) in wraps.into_iter().zip(process_outlets) {
            let receiver_inlet = receiver_inlet.clone();
            let acknowledgements_inlets = acknowledgements_inlets.clone();

            fuse.spawn(async move {
                let _ = DatagramDispatcher::process(
                    wrap,
                    process_outlet,
                    receiver_inlet,
                    acknowledgements_inlets,
                )
                .await;
            });
        }

        // Sender and receiver

        let sender = DatagramSender::new(route_out_inlets, fuse.clone());
        let receiver = DatagramReceiver::new(receiver_outlet, fuse.clone());

        Ok(DatagramDispatcher { sender, receiver })
    }

    async fn route_out(
        index: u8,
        wrap: Arc<UdpWrap>,
        mut route_out_outlet: DatagramOutlet,
        mut acknowledgements_outlet: AcknowledgementsOutlet,
    ) {
        // Data structures

        let mut sequence: u64 = 0;
        let mut datagrams: HashMap<u64, (SocketAddr, Vec<u8>)> = HashMap::new();
        let mut retransmissions: VecDeque<(Instant, u64)> = VecDeque::new();

        // Buffers

        let mut route_out_buffer: Vec<(SocketAddr, Vec<u8>)> = Vec::new();
        let mut acknowledgements_buffer: Vec<u64> = Vec::new();
        let mut transmission_buffer: Vec<(u64, (SocketAddr, Vec<u8>))> = Vec::new();

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
                _ = time::sleep(Duration::from_millis(10)), if !retransmissions.is_empty() => {} // TODO: Add settings
            }

            // Flush outlets

            while let Ok(datagram) = route_out_outlet.try_recv() {
                route_out_buffer.push(datagram);
            }

            while let Ok(mut acknowledgements) = acknowledgements_outlet.try_recv() {
                acknowledgements_buffer.append(&mut acknowledgements);
            }

            // Remove acknowledged `datagrams`

            for acknowledgement in acknowledgements_buffer.drain(..) {
                datagrams.remove(&acknowledgement);
            }

            // Stage `retransmissions` for transmission

            let now = Instant::now();

            loop {
                if let Some((time, _)) = retransmissions.front() {
                    if *time > now {
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

            let send_multiple_iterator = transmission_buffer
                .iter()
                .map(|(_, (address, buffer))| (address, buffer.as_slice()));

            // TODO: Retransmit if some packets are not sent (this is indicated
            // by the `Ok` variant of the value returned by `wrap.send_multiple(..)`)
            let _ = wrap.send_multiple(send_multiple_iterator).await;

            // Populate `datagrams` and `retransmissions`

            // TODO: Add settings
            let retransmission_time = Instant::now() + Duration::from_millis(50);

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
    ) {
        let mut acknowledgements_out: Vec<(SocketAddr, [u8; 10])> = Vec::new();
        let mut acknowledgements_in: Vec<Vec<u64>> =
            vec![Vec::new(); acknowledgements_inlets.len()];

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

            let send_multiple_iterator = acknowledgements_out
                .iter()
                .map(|(address, buffer)| (address, buffer.as_slice()));

            // TODO: Retransmit if some packets are not sent (this is indicated
            // by the `Ok` variant of the value returned by `wrap.send_multiple(..)`)
            let _ = wrap.send_multiple(send_multiple_iterator).await;

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
