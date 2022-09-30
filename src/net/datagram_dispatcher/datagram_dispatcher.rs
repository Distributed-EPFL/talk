use atomic_counter::{AtomicCounter, RelaxedCounter};
use socket2::Domain;

use crate::{
    net::{
        datagram_dispatcher::{
            DatagramDispatcherSettings, DatagramReceiver, DatagramSender, DatagramTable, Message,
            Statistics, MAXIMUM_TRANSMISSION_UNIT,
        },
        Message as NetMessage,
    },
    sync::fuse::{Fuse, Relay},
};

use doomstack::{here, Doom, ResultExt, Top};

use std::{
    cmp,
    collections::VecDeque,
    io::{self, ErrorKind},
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::{Arc, Mutex},
};

use tokio::{
    task, time,
    time::{Duration, Instant},
};

use flume::TrySendError;

#[cfg(target_os = "linux")]
use {
    nix::sys::socket::{recvmmsg, sendmmsg, MsgFlags, RecvMmsgData, SendMmsgData, SockaddrStorage},
    std::convert::TryInto,
    std::io::{IoSlice, IoSliceMut},
    std::net::{Ipv4Addr, SocketAddrV4},
    std::os::unix::io::AsRawFd,
};

type MessageInlet<M> = flume::Sender<(SocketAddr, M)>;
type MessageOutlet<M> = flume::Receiver<(SocketAddr, M)>;

type DatagramInlet = flume::Sender<(SocketAddr, Message)>;
type DatagramOutlet = flume::Receiver<(SocketAddr, Message)>;

type AcknowledgementInlet = flume::Sender<(SocketAddr, usize)>;
type AcknowledgementOutlet = flume::Receiver<(SocketAddr, usize)>;

type CompletionInlet = flume::Sender<usize>;
type CompletionOutlet = flume::Receiver<usize>;

pub struct DatagramDispatcher<S: NetMessage, R: NetMessage> {
    sender: DatagramSender<S>,
    receiver: DatagramReceiver<R>,
}

#[derive(Doom)]
pub enum DatagramDispatcherError {
    #[doom(description("Failed to bind address: {:?}", source))]
    #[doom(wrap(bind_failed))]
    BindFailed { source: io::Error },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PaceOutPoll {
    Complete,
    Acknowledge,
    Resend,
    Send,
}

enum PaceOutTask {
    Complete(usize),
    Acknowledge(SocketAddr, usize),
    Resend(usize),
    Send(SocketAddr, Message),
}

impl<S, R> DatagramDispatcher<S, R>
where
    S: NetMessage,
    R: NetMessage,
{
    pub fn bind<A>(
        address: A,
        settings: DatagramDispatcherSettings,
    ) -> Result<Self, Top<DatagramDispatcherError>>
    where
        A: ToSocketAddrs,
    {
        let (receive_inlet, receive_outlet) = flume::bounded(settings.receive_channel_capacity);

        let (process_in_inlet, process_in_outlet) =
            flume::bounded(settings.process_in_channel_capacity);

        let (process_out_inlet, process_out_outlet) =
            flume::bounded(settings.process_out_channel_capacity);

        let (pace_out_datagram_inlet, pace_out_datagram_outlet) =
            flume::bounded(settings.pace_out_datagram_channel_capacity);

        let (pace_out_acknowledgement_inlet, pace_out_acknowledgement_outlet) =
            flume::bounded(settings.pace_out_acknowledgement_channel_capacity);

        let (pace_out_completion_inlet, pace_out_completion_outlet) =
            flume::bounded(settings.pace_out_completion_channel_capacity);

        let (route_out_inlet, route_out_outlet) =
            flume::bounded(settings.route_out_channel_capacity);

        let statistics = Statistics {
            packets_sent: RelaxedCounter::new(0),
            packets_received: RelaxedCounter::new(0),
            message_packets_processed: RelaxedCounter::new(0),
            acknowledgement_packets_processed: RelaxedCounter::new(0),
            retransmissions: RelaxedCounter::new(0),
            pace_out_chokes: RelaxedCounter::new(0),
            process_in_drops: RelaxedCounter::new(0),
            route_out_drops: RelaxedCounter::new(0),
        };

        let statistics = Arc::new(statistics);

        let fuse = Fuse::new();

        let sockets: Vec<Arc<UdpSocket>> = (0..settings.sockets)
            .map(|_| {
                address
                    .to_socket_addrs()
                    .and_then(|mut iter| {
                        iter.next()
                            .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "no addr found"))
                    })
                    .and_then(|bind_addr| {
                        socket2::Socket::new(
                            Domain::for_address(bind_addr),
                            socket2::Type::DGRAM,
                            None,
                        )
                        .and_then(|socket| {
                            socket.set_reuse_port(true)?;
                            socket.bind(&bind_addr.into())?;
                            Ok(Arc::new(socket.into()))
                        })
                    })
                    .map_err(DatagramDispatcherError::bind_failed)
                    .map_err(DatagramDispatcherError::into_top)
                    .spot(here!())
            })
            .collect::<Result<_, _>>()?;

        for socket in sockets.iter().cloned() {
            let settings = settings.clone();
            let statistics = statistics.clone();
            let relay = fuse.relay();
            let process_in_inlet = process_in_inlet.clone();

            task::spawn_blocking(move || {
                DatagramDispatcher::<S, R>::route_in(
                    &socket,
                    process_in_inlet,
                    settings,
                    statistics,
                    relay,
                )
            });
        }

        for _ in 0..settings.process_in_tasks {
            let statistics = statistics.clone();
            let process_in_outlet = process_in_outlet.clone();

            fuse.spawn(DatagramDispatcher::<S, R>::process_in(
                process_in_outlet,
                receive_inlet.clone(),
                pace_out_acknowledgement_inlet.clone(),
                pace_out_completion_inlet.clone(),
                statistics,
            ));
        }

        for _ in 0..settings.process_out_tasks {
            let relay = fuse.relay();
            let process_out_outlet = process_out_outlet.clone();
            fuse.spawn(DatagramDispatcher::<S, R>::process_out(
                process_out_outlet,
                pace_out_datagram_inlet.clone(),
                relay,
            ));
        }

        let datagram_table = Arc::new(Mutex::new(DatagramTable::new()));

        for _ in 0..settings.pace_out_tasks {
            let statistics = statistics.clone();
            let settings = settings.clone();

            fuse.spawn(DatagramDispatcher::<S, R>::pace_out(
                datagram_table.clone(),
                pace_out_datagram_outlet.clone(),
                pace_out_acknowledgement_outlet.clone(),
                pace_out_completion_outlet.clone(),
                route_out_inlet.clone(),
                statistics,
                settings,
            ));
        }

        for socket in sockets {
            let settings = settings.clone();
            let statistics = statistics.clone();
            let route_out_outlet = route_out_outlet.clone();

            task::spawn_blocking(move || {
                DatagramDispatcher::<S, R>::route_out(
                    &socket,
                    route_out_outlet,
                    settings,
                    statistics,
                )
            });
        }

        let fuse = Arc::new(fuse);

        let sender = DatagramSender::new(
            process_out_inlet,
            settings,
            statistics.clone(),
            fuse.clone(),
        );

        let receiver = DatagramReceiver::new(receive_outlet, statistics, fuse);

        Ok(DatagramDispatcher { sender, receiver })
    }

    pub async fn send(&self, destination: SocketAddr, payload: S) {
        self.sender.send(destination, payload).await;
    }

    pub async fn receive(&mut self) -> (SocketAddr, R) {
        self.receiver.receive().await
    }

    pub async fn pace<I>(&self, datagrams: I, rate: f64)
    where
        I: IntoIterator<Item = (SocketAddr, S)>,
    {
        self.sender.pace(datagrams, rate).await
    }

    pub fn packets_sent(&self) -> usize {
        self.sender.packets_sent()
    }

    pub fn packets_received(&self) -> usize {
        self.sender.packets_received()
    }

    pub fn message_packets_processed(&self) -> usize {
        self.sender.message_packets_processed()
    }

    pub fn acknowledgement_packets_processed(&self) -> usize {
        self.sender.acknowledgement_packets_processed()
    }

    pub fn retransmissions(&self) -> usize {
        self.sender.retransmissions()
    }

    pub fn pace_out_chokes(&self) -> usize {
        self.sender.pace_out_chokes()
    }

    pub fn process_in_drops(&self) -> usize {
        self.sender.process_in_drops()
    }

    pub fn route_out_drops(&self) -> usize {
        self.sender.route_out_drops()
    }

    pub fn split(self) -> (DatagramSender<S>, DatagramReceiver<R>) {
        (self.sender, self.receiver)
    }

    #[cfg(not(target_os = "linux"))]
    fn route_in(
        socket: &UdpSocket,
        process_in_inlet: DatagramInlet,
        _settings: DatagramDispatcherSettings,
        statistics: Arc<Statistics>,
        mut relay: Relay,
    ) {
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap(); // TODO: Determine if this call can fail

        loop {
            let mut buffer = [0u8; MAXIMUM_TRANSMISSION_UNIT];

            let datagram = socket.recv_from(&mut buffer);

            if !relay.is_on() {
                break;
            }

            let (size, source) = match datagram {
                Ok((size, source)) => {
                    statistics.packets_received.inc();
                    (size, source)
                }
                Err(error) => {
                    if error.kind() == io::ErrorKind::WouldBlock
                        || error.kind() == io::ErrorKind::TimedOut
                    {
                        continue;
                    } else {
                        panic!("unexpected `ErrorKind` when `recv_from`ing");
                    }
                }
            };

            let message = Message { buffer, size };

            if let Err(TrySendError::Full(..)) = process_in_inlet.try_send((source, message)) {
                // `process_in` is too busy: just drop the packet
                statistics.process_in_drops.inc();
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn route_in(
        socket: &UdpSocket,
        process_in_inlet: DatagramInlet,
        settings: DatagramDispatcherSettings,
        statistics: Arc<Statistics>,
        mut relay: Relay,
    ) {
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap(); // TODO: Determine if this call can fail

        let descriptor = socket.as_raw_fd();
        let mut buffer = vec![0u8; MAXIMUM_TRANSMISSION_UNIT * settings.route_in_batch_size];

        loop {
            let mut data = buffer
                .chunks_exact_mut(MAXIMUM_TRANSMISSION_UNIT)
                .map(|chunk| RecvMmsgData {
                    iov: [IoSliceMut::new(chunk)],
                    cmsg_buffer: None,
                })
                .collect::<Vec<RecvMmsgData<_>>>();

            let messages_res = recvmmsg(
                descriptor,
                &mut data,
                unsafe {
                    // unsafe required as MsgFlags enum does not include `MSG_WAITFORONE`
                    MsgFlags::from_bits_unchecked(nix::libc::MSG_WAITFORONE)
                },
                None,
            );

            if !relay.is_on() {
                break;
            }

            let datagrams: Vec<_> = match messages_res {
                Ok(datagrams) => {
                    statistics.packets_received.add(datagrams.len());
                    datagrams
                        .iter()
                        .map(|message| {
                            let sockaddr_storage: SockaddrStorage = message.address.unwrap();
                            let sockaddr_in = sockaddr_storage.as_sockaddr_in().unwrap();

                            let socketaddr_v4 = SocketAddrV4::new(
                                Ipv4Addr::from(sockaddr_in.ip()),
                                sockaddr_in.port(),
                            );

                            let address = SocketAddr::V4(socketaddr_v4);

                            (address, message.bytes)
                        })
                        .collect()
                }
                Err(error) => {
                    if error == nix::errno::Errno::EWOULDBLOCK
                        || error == nix::errno::Errno::ETIMEDOUT
                    {
                        continue;
                    } else {
                        panic!("unexpected `ErrorKind` when `recv_from`ing");
                    }
                }
            };

            for (source, message) in datagrams
                .iter()
                .zip(buffer.chunks_exact(MAXIMUM_TRANSMISSION_UNIT))
                .map(|((address, size), buffer)| {
                    (
                        address,
                        Message {
                            buffer: buffer.try_into().expect("fixed size buffer"),
                            size: *size,
                        },
                    )
                })
            {
                if let Err(TrySendError::Full(..)) = process_in_inlet.try_send((*source, message)) {
                    // `process_in` is too busy: just drop the packet
                    statistics.process_in_drops.inc();
                }
            }
        }
    }

    async fn process_in(
        process_in_outlet: DatagramOutlet,
        receive_inlet: MessageInlet<R>,
        pace_out_acknowledgement_inlet: AcknowledgementInlet,
        pace_out_completion_inlet: CompletionInlet,
        statistics: Arc<Statistics>,
    ) {
        loop {
            let (source, message) = if let Ok(datagram) = process_in_outlet.recv_async().await {
                datagram
            } else {
                // `DatagramDispatcher` has dropped, shutdown
                return;
            };

            if message.size < 9 {
                continue;
            }

            let (payload, footer) = message.buffer[..message.size].split_at(message.size - 9);

            let mut index = [0u8; 8];
            index.clone_from_slice(&footer[1..]);
            let index = u64::from_le_bytes(index) as usize;

            if footer[0] == 0 {
                // First footer byte is 0: MESSAGE

                if let Ok(payload) = bincode::deserialize::<R>(payload) {
                    let _ = pace_out_acknowledgement_inlet
                        .send_async((source, index))
                        .await;
                    let _ = receive_inlet.send_async((source, payload)).await;
                    statistics.message_packets_processed.inc();
                } else {
                    println!("Failed to deserialize...??");
                }
            } else if footer[0] == 1 {
                // First footer byte is 1: ACKNOWLEDGEMENT

                let _ = pace_out_completion_inlet.send_async(index).await;
                statistics.acknowledgement_packets_processed.inc();
            }
        }
    }

    async fn process_out(
        process_out_outlet: MessageOutlet<S>,
        pace_out_datagram_inlet: DatagramInlet,
        mut relay: Relay,
    ) {
        loop {
            let (destination, payload) = if let Ok(message) = process_out_outlet.recv_async().await
            {
                message
            } else {
                // The Sender has been dropped. Returning immediately would drop
                // `pace_out_datagram_inlet` and break the `wait_pace_out_task`.
                // Let's wait for the receiver to shutdown too.
                relay.wait().await;
                return;
            };

            let mut buffer = [0u8; MAXIMUM_TRANSMISSION_UNIT];
            let mut write = &mut buffer[..];

            if bincode::serialize_into(&mut write, &payload).is_ok() {
                let size = MAXIMUM_TRANSMISSION_UNIT - write.len();
                let message = Message { buffer, size };
                let _ = pace_out_datagram_inlet
                    .send_async((destination, message))
                    .await;
            }
        }
    }

    async fn pace_out(
        datagram_table: Arc<Mutex<DatagramTable>>,
        mut pace_out_datagram_outlet: DatagramOutlet,
        mut pace_out_acknowledgement_outlet: AcknowledgementOutlet,
        mut pace_out_completion_outlet: CompletionOutlet,
        route_out_inlet: DatagramInlet,
        statistics: Arc<Statistics>,
        settings: DatagramDispatcherSettings,
    ) {
        let mut retransmission_queue = VecDeque::new();

        let mut last_tick = Instant::now();

        loop {
            let sleep_time = settings
                .minimum_rate_window
                .saturating_sub(last_tick.elapsed());

            if sleep_time > Duration::ZERO {
                time::sleep(sleep_time).await;
            } else {
                statistics.pace_out_chokes.inc();
            }

            let task = if let Some(task) = DatagramDispatcher::<S, R>::wait_pace_out_task(
                &mut pace_out_datagram_outlet,
                &mut pace_out_acknowledgement_outlet,
                &mut pace_out_completion_outlet,
                &mut retransmission_queue,
            )
            .await
            {
                task
            } else {
                // `DatagramDispatcher` has dropped, shutdown
                return;
            };

            let window = last_tick.elapsed();
            last_tick = Instant::now();

            let mut poll = task.poll();

            let packet_allowance = (cmp::min(window, settings.maximum_rate_window).as_secs_f64()
                * settings.maximum_packet_rate
                / settings.pace_out_tasks as f64) as usize;

            let mut packets_sent = 0;

            if DatagramDispatcher::<S, R>::fulfill_pace_out_task(
                &datagram_table,
                &mut retransmission_queue,
                task,
                &route_out_inlet,
                statistics.as_ref(),
                &settings,
            ) {
                packets_sent += 1;
            }

            while packets_sent < packet_allowance {
                let task = if let Some(task) = DatagramDispatcher::<S, R>::next_pace_out_task(
                    poll,
                    &mut pace_out_datagram_outlet,
                    &mut pace_out_acknowledgement_outlet,
                    &mut pace_out_completion_outlet,
                    &mut retransmission_queue,
                ) {
                    task
                } else {
                    // No more tasks to fulfill on the fly
                    break;
                };

                poll = task.poll();

                if DatagramDispatcher::<S, R>::fulfill_pace_out_task(
                    &datagram_table,
                    &mut retransmission_queue,
                    task,
                    &route_out_inlet,
                    statistics.as_ref(),
                    &settings,
                ) {
                    packets_sent += 1;
                }
            }
        }
    }

    async fn wait_pace_out_task(
        pace_out_datagram_outlet: &mut DatagramOutlet,
        pace_out_acknowledgement_outlet: &mut AcknowledgementOutlet,
        pace_out_completion_outlet: &mut CompletionOutlet,
        retransmission_queue: &mut VecDeque<(Instant, usize)>,
    ) -> Option<PaceOutTask> {
        let next_retransmission = retransmission_queue
            .front()
            .map(|(next_retransmission, _)| *next_retransmission);

        // This is a `Future` that waits until `next_retransmission` iff
        // `next_retransmission` is `Some`. This is necessary because
        // `tokio::select!` branches with an `, if..` clause do not guarantee that
        // the selected future will be evaulated only if the `, if..` clause
        // is satisfied. As a result, an `, if next_retransmission.is_some()` clause
        // would not be sufficient to prevent a `next_retransmission.unwrap()` from
        // panicking in a `tokio::select!` branch. Note that this future is not
        // polled unless `next_retransmission` is `Some`.
        let wait_until_next_retransmission = async {
            if let Some(next_retransmission) = next_retransmission {
                time::sleep_until(next_retransmission).await;
            };
        };

        tokio::select! {
            biased;

            completion = pace_out_completion_outlet.recv_async() => {
                completion.map(PaceOutTask::Complete).ok()
            }
            acknowledgement = pace_out_acknowledgement_outlet.recv_async() => {
                acknowledgement.map(|(destination, index)| PaceOutTask::Acknowledge(destination, index)).ok()
            },
            _ = wait_until_next_retransmission, if next_retransmission.is_some() => {
                let (_, index) = retransmission_queue.pop_front().unwrap();
                Some(PaceOutTask::Resend(index))
            },
            datagram = pace_out_datagram_outlet.recv_async() => {
                datagram.map(|(destination, message)| PaceOutTask::Send(destination, message)).ok()
            },
        }
    }

    fn next_pace_out_task(
        poll: PaceOutPoll,
        pace_out_datagram_outlet: &mut DatagramOutlet,
        pace_out_acknowledgement_outlet: &mut AcknowledgementOutlet,
        pace_out_completion_outlet: &mut CompletionOutlet,
        retransmission_queue: &mut VecDeque<(Instant, usize)>,
    ) -> Option<PaceOutTask> {
        if poll <= PaceOutPoll::Complete {
            if let Ok(index) = pace_out_completion_outlet.try_recv() {
                return Some(PaceOutTask::Complete(index));
            }
        }

        if poll <= PaceOutPoll::Acknowledge {
            if let Ok((destination, index)) = pace_out_acknowledgement_outlet.try_recv() {
                return Some(PaceOutTask::Acknowledge(destination, index));
            }
        }

        if poll <= PaceOutPoll::Resend {
            if let Some((next_retransmission, _)) = retransmission_queue.front() {
                if Instant::now() >= *next_retransmission {
                    let (_, index) = retransmission_queue.pop_front().unwrap();
                    return Some(PaceOutTask::Resend(index));
                }
            }
        }

        if poll <= PaceOutPoll::Send {
            if let Ok((destination, message)) = pace_out_datagram_outlet.try_recv() {
                return Some(PaceOutTask::Send(destination, message));
            }
        }

        None
    }

    fn fulfill_pace_out_task(
        datagram_table: &Mutex<DatagramTable>,
        retransmission_queue: &mut VecDeque<(Instant, usize)>,
        task: PaceOutTask,
        route_out_inlet: &DatagramInlet,
        statistics: &Statistics,
        settings: &DatagramDispatcherSettings,
    ) -> bool {
        match task {
            PaceOutTask::Complete(index) => {
                let mut datagram_table = datagram_table.lock().unwrap();
                datagram_table.remove(index);
                false
            }
            PaceOutTask::Acknowledge(destination, index) => {
                let mut buffer = [1u8; 2048]; // First footer byte is 1: ACKNOWLEDGEMENT (note that the following bytes are overwritten)
                buffer[1..9].clone_from_slice((index as u64).to_le_bytes().as_slice());

                let message = Message { buffer, size: 9 };

                if let Err(TrySendError::Full(..)) =
                    route_out_inlet.try_send((destination, message))
                {
                    // `route_out` is too busy: just drop the packet
                    statistics.route_out_drops.inc();
                }

                true
            }
            PaceOutTask::Resend(index) => {
                let datagram_table = datagram_table.lock().unwrap();
                if let Some((destination, message)) = datagram_table.get(index) {
                    if let Err(TrySendError::Full(..)) =
                        route_out_inlet.try_send((*destination, message.clone()))
                    {
                        // `route_out` is too busy: just drop the packet
                        statistics.route_out_drops.inc();
                    }

                    retransmission_queue
                        .push_back((Instant::now() + settings.retransmission_delay, index));

                    statistics.retransmissions.inc();

                    true
                } else {
                    false
                }
            }
            PaceOutTask::Send(destination, message) => {
                let message_with_footer = |mut message: Message, index| {
                    message.buffer[message.size] = 0; // First footer byte is 0: MESSAGE
                    message.buffer[(message.size + 1)..(message.size + 9)]
                        .clone_from_slice((index as u64).to_le_bytes().as_slice());
                    message.size += 9;
                    message
                };

                // The message is copied before the footer is written, to avoid holding the mutex while cloning.
                let message2 = message.clone();

                let index = {
                    let mut datagram_table = datagram_table.lock().unwrap();
                    let index = datagram_table.cursor();
                    datagram_table.push(destination, message_with_footer(message, index));
                    index
                };

                if let Err(TrySendError::Full(..)) =
                    route_out_inlet.try_send((destination, message_with_footer(message2, index)))
                {
                    // `route_out` is too busy: just drop the packet
                    statistics.route_out_drops.inc();
                }

                retransmission_queue
                    .push_back((Instant::now() + settings.retransmission_delay, index));

                true
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn route_out(
        socket: &UdpSocket,
        route_out_outlet: DatagramOutlet,
        _settings: DatagramDispatcherSettings,
        statistics: Arc<Statistics>,
    ) {
        loop {
            let (destination, message) = if let Ok(datagram) = route_out_outlet.recv() {
                datagram
            } else {
                // `DatagramDispatcher` has dropped, shutdown
                return;
            };

            socket
                .send_to(&message.buffer[..message.size], destination)
                .unwrap(); // TODO: Determine if this can fail

            statistics.packets_sent.inc();
        }
    }

    #[cfg(target_os = "linux")]
    fn route_out(
        socket: &UdpSocket,
        route_out_outlet: DatagramOutlet,
        settings: DatagramDispatcherSettings,
        statistics: Arc<Statistics>,
    ) {
        let mut batch: Vec<(SocketAddr, Message)> =
            Vec::with_capacity(settings.route_out_batch_size);

        loop {
            batch.clear();

            if let Ok(datagram) = route_out_outlet.recv() {
                batch.push(datagram)
            } else {
                // `DatagramDispatcher` has dropped, shutdown
                return;
            };

            for _ in 1..settings.route_out_batch_size {
                match route_out_outlet.try_recv() {
                    Ok(msg) => {
                        batch.push(msg);
                    }
                    Err(TryRecvError::Empty) => {
                        // No more datagrams can be pulled on the fly, finalize `batch`.
                        break;
                    }
                    Err(TryRecvError::Disconnected) => {
                        // `DatagramDispatcher` has dropped, shutdown
                        return;
                    }
                }
            }

            let data = batch
                .iter()
                .map(|(address, message)| {
                    let address: SockaddrStorage = (*address).into();
                    SendMmsgData {
                        iov: [IoSlice::new(&message.buffer[..message.size])],
                        cmsgs: &[],
                        addr: Some(address),
                        _lt: Default::default(),
                    }
                })
                .collect::<Vec<SendMmsgData<_, _, _>>>();

            let sent = {
                let descriptor = socket.as_raw_fd();
                let sizes = sendmmsg(descriptor, &data, MsgFlags::empty()).unwrap();
                let sent = sizes
                    .iter()
                    .position(|size| *size == 0)
                    .unwrap_or(sizes.len());
                sent
            };

            statistics.packets_sent.add(sent);
        }
    }
}

impl PaceOutTask {
    fn poll(&self) -> PaceOutPoll {
        match self {
            PaceOutTask::Complete(..) => PaceOutPoll::Complete,
            PaceOutTask::Acknowledge(..) => PaceOutPoll::Acknowledge,
            PaceOutTask::Resend(..) => PaceOutPoll::Resend,
            PaceOutTask::Send(..) => PaceOutPoll::Send,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn single() {
        let receiver = tokio::spawn(async {
            let mut dispatcher =
                DatagramDispatcher::<u64, u64>::bind("127.0.0.1:1260", Default::default()).unwrap();
            let (_, value) = dispatcher.receive().await;
            assert_eq!(value, 42);
        });

        let dispatcher =
            DatagramDispatcher::<u64, u64>::bind("127.0.0.1:0", Default::default()).unwrap();
        dispatcher.send("127.0.0.1:1260".parse().unwrap(), 42).await;
        receiver.await.unwrap();
    }

    #[tokio::test]
    async fn acknowledge_after_sender_dropped() {
        let receiver = tokio::spawn(async {
            let dispatcher =
                DatagramDispatcher::<u64, u64>::bind("127.0.0.1:1261", Default::default()).unwrap();
            let (sender, mut receiver) = dispatcher.split();
            drop(sender);
            let (_, value) = receiver.receive().await;
            assert_eq!(value, 42);
            time::sleep(Duration::from_millis(100)).await;
        });

        let dispatcher =
            DatagramDispatcher::<u64, u64>::bind("127.0.0.1:0", Default::default()).unwrap();
        dispatcher.send("127.0.0.1:1261".parse().unwrap(), 42).await;
        receiver.await.unwrap();
        assert_eq!(dispatcher.receiver.acknowledgement_packets_processed(), 1);
    }
}
