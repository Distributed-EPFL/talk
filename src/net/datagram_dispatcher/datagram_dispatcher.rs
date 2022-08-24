use atomic_counter::{AtomicCounter, RelaxedCounter};

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
    io,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{
    sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender},
    task, time,
};

type MessageInlet<M> = MpscSender<(SocketAddr, M)>;
type MessageOutlet<M> = MpscReceiver<(SocketAddr, M)>;

type DatagramInlet = MpscSender<(SocketAddr, Message)>;
type DatagramOutlet = MpscReceiver<(SocketAddr, Message)>;

type AcknowledgementInlet = MpscSender<(SocketAddr, usize)>;
type AcknowledgementOutlet = MpscReceiver<(SocketAddr, usize)>;

type CompletionInlet = MpscSender<usize>;
type CompletionOutlet = MpscReceiver<usize>;

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
        let socket = UdpSocket::bind(address)
            .map_err(DatagramDispatcherError::bind_failed)
            .map_err(DatagramDispatcherError::into_top)
            .spot(here!())?;

        let socket = Arc::new(socket);

        let (receive_inlet, receive_outlet) = mpsc::channel(settings.receive_channel_capacity);

        let mut process_in_inlets = Vec::new();
        let mut process_in_outlets = Vec::new();

        for _ in 0..settings.process_in_tasks {
            let (process_in_inlet, process_in_outlet) =
                mpsc::channel(settings.process_in_channel_capacity);

            process_in_inlets.push(process_in_inlet);
            process_in_outlets.push(process_in_outlet);
        }

        let mut process_out_inlets = Vec::new();
        let mut process_out_outlets = Vec::new();

        for _ in 0..settings.process_out_tasks {
            let (process_out_inlet, process_out_outlet) =
                mpsc::channel(settings.process_out_channel_capacity);

            process_out_inlets.push(process_out_inlet);
            process_out_outlets.push(process_out_outlet);
        }

        let (pace_out_datagram_inlet, pace_out_datagram_outlet) =
            mpsc::channel(settings.pace_out_datagram_channel_capacity);

        let (pace_out_acknowledgement_inlet, pace_out_acknowledgement_outlet) =
            mpsc::channel(settings.pace_out_acknowledgement_channel_capacity);

        let (pace_out_completion_inlet, pace_out_completion_outlet) =
            mpsc::channel(settings.pace_out_completion_channel_capacity);

        let (route_out_inlet, route_out_outlet) =
            mpsc::channel(settings.route_out_channel_capacity);

        let statistics = Statistics {
            retransmissions: RelaxedCounter::new(0),
            process_in_drops: RelaxedCounter::new(0),
        };

        let statistics = Arc::new(statistics);

        let fuse = Fuse::new();

        {
            let socket = socket.clone();
            let statistics = statistics.clone();
            let settings = settings.clone();
            let relay = fuse.relay();

            task::spawn_blocking(move || {
                DatagramDispatcher::<S, R>::route_in(
                    socket,
                    process_in_inlets,
                    statistics,
                    settings,
                    relay,
                )
            });
        }

        for process_in_outlet in process_in_outlets {
            fuse.spawn(DatagramDispatcher::<S, R>::process_in(
                process_in_outlet,
                receive_inlet.clone(),
                pace_out_acknowledgement_inlet.clone(),
                pace_out_completion_inlet.clone(),
            ));
        }

        for process_out_outlet in process_out_outlets {
            fuse.spawn(DatagramDispatcher::<S, R>::process_out(
                process_out_outlet,
                pace_out_datagram_inlet.clone(),
            ));
        }

        {
            let statistics = statistics.clone();
            let settings = settings.clone();

            fuse.spawn(DatagramDispatcher::<S, R>::pace_out(
                pace_out_datagram_outlet,
                pace_out_acknowledgement_outlet,
                pace_out_completion_outlet,
                route_out_inlet,
                statistics,
                settings,
            ));
        }

        {
            let socket = socket.clone();

            task::spawn_blocking(move || {
                DatagramDispatcher::<S, R>::route_out(socket, route_out_outlet)
            });
        }

        let fuse = Arc::new(fuse);

        let sender = DatagramSender::new(process_out_inlets, statistics.clone(), fuse.clone());
        let receiver = DatagramReceiver::new(receive_outlet, statistics.clone(), fuse);

        Ok(DatagramDispatcher { sender, receiver })
    }

    pub async fn send(&self, destination: SocketAddr, payload: S) {
        self.sender.send(destination, payload).await;
    }

    pub async fn receive(&mut self) -> (SocketAddr, R) {
        self.receiver.receive().await
    }

    pub fn retransmissions(&self) -> usize {
        self.sender.retransmissions()
    }

    pub fn process_in_drops(&self) -> usize {
        self.sender.process_in_drops()
    }

    pub fn split(self) -> (DatagramSender<S>, DatagramReceiver<R>) {
        (self.sender, self.receiver)
    }

    fn route_in(
        socket: Arc<UdpSocket>,
        process_in_inlets: Vec<DatagramInlet>,
        statistics: Arc<Statistics>,
        settings: DatagramDispatcherSettings,
        mut relay: Relay,
    ) {
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap(); // TODO: Determine if this call can fail

        for robin in 0.. {
            let mut buffer = [0u8; MAXIMUM_TRANSMISSION_UNIT];

            let datagram = socket.recv_from(&mut buffer);

            if !relay.is_on() {
                break;
            }

            let (size, source) = match datagram {
                Ok((size, source)) => (size, source),
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

            if process_in_inlets
                .get(robin % settings.process_in_tasks)
                .unwrap()
                .try_send((source, message))
                .is_err()
            {
                statistics.process_in_drops.inc();
            }
        }
    }

    async fn process_in(
        mut process_in_outlet: DatagramOutlet,
        receive_inlet: MessageInlet<R>,
        pace_out_acknowledgement_inlet: AcknowledgementInlet,
        pace_out_completion_inlet: CompletionInlet,
    ) {
        loop {
            let (source, message) = if let Some(datagram) = process_in_outlet.recv().await {
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

                let _ = pace_out_acknowledgement_inlet.send((source, index)).await;

                if let Ok(payload) = bincode::deserialize::<R>(payload) {
                    let _ = receive_inlet.send((source, payload)).await;
                }
            } else if footer[0] == 1 {
                // First footer byte is 1: ACKNOWLEDGEMENT

                let _ = pace_out_completion_inlet.send(index).await;
            }
        }
    }

    async fn process_out(
        mut process_out_outlet: MessageOutlet<S>,
        pace_out_datagram_inlet: DatagramInlet,
    ) {
        loop {
            let (destination, payload) = if let Some(message) = process_out_outlet.recv().await {
                message
            } else {
                // `DatagramDispatcher` has dropped, shutdown
                return;
            };

            let mut buffer = [0u8; MAXIMUM_TRANSMISSION_UNIT];
            let mut write = &mut buffer[..];

            if bincode::serialize_into(&mut write, &payload).is_ok() {
                let size = MAXIMUM_TRANSMISSION_UNIT - write.len();
                let message = Message { buffer, size };
                let _ = pace_out_datagram_inlet.send((destination, message)).await;
            }
        }
    }

    async fn pace_out(
        mut pace_out_datagram_outlet: DatagramOutlet,
        mut pace_out_acknowledgement_outlet: AcknowledgementOutlet,
        mut pace_out_completion_outlet: CompletionOutlet,
        route_out_inlet: DatagramInlet,
        statistics: Arc<Statistics>,
        settings: DatagramDispatcherSettings,
    ) {
        let mut cursor = 0;

        let mut datagram_table = DatagramTable::new();
        let mut retransmission_queue = VecDeque::new();

        let mut last_tick = Instant::now();

        loop {
            time::sleep(
                settings
                    .minimum_rate_window
                    .saturating_sub(last_tick.elapsed()),
            )
            .await;

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
                * settings.maximum_packet_rate) as usize;

            let mut packets_sent = 0;

            if DatagramDispatcher::<S, R>::fulfill_pace_out_task(
                &mut cursor,
                &mut datagram_table,
                &mut retransmission_queue,
                task,
                &route_out_inlet,
                statistics.as_ref(),
                &settings,
            )
            .await
            {
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
                    &mut cursor,
                    &mut datagram_table,
                    &mut retransmission_queue,
                    task,
                    &route_out_inlet,
                    statistics.as_ref(),
                    &settings,
                )
                .await
                {
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
                time::sleep(next_retransmission - Instant::now()).await;
            };
        };

        tokio::select! {
            biased;

            completion = pace_out_completion_outlet.recv() => {
                completion.map(|index| PaceOutTask::Complete(index))
            }
            acknowledgement = pace_out_acknowledgement_outlet.recv() => {
                acknowledgement.map(|(destination, index)| PaceOutTask::Acknowledge(destination, index))
            },
            _ = wait_until_next_retransmission, if next_retransmission.is_some() => {
                let (_, index) = retransmission_queue.pop_front().unwrap();
                Some(PaceOutTask::Resend(index))
            },
            datagram = pace_out_datagram_outlet.recv() => {
                datagram.map(|(destination, message)| PaceOutTask::Send(destination, message))
            }
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

    async fn fulfill_pace_out_task(
        cursor: &mut usize,
        datagram_table: &mut DatagramTable,
        retransmission_queue: &mut VecDeque<(Instant, usize)>,
        task: PaceOutTask,
        route_out_inlet: &DatagramInlet,
        statistics: &Statistics,
        settings: &DatagramDispatcherSettings,
    ) -> bool {
        match task {
            PaceOutTask::Complete(index) => {
                datagram_table.remove(index);
                false
            }
            PaceOutTask::Acknowledge(destination, index) => {
                let mut buffer = [1u8; 2048]; // First footer byte is 1: ACKNOWLEDGEMENT (note that the following bytes are overwritten)
                buffer[1..9].clone_from_slice((index as u64).to_le_bytes().as_slice());

                let message = Message { buffer, size: 9 };
                let _ = route_out_inlet.send((destination, message)).await;

                true
            }
            PaceOutTask::Resend(index) => {
                if let Some((destination, message)) = datagram_table.get(index) {
                    let _ = route_out_inlet.send((destination.clone(), message.clone()));

                    retransmission_queue
                        .push_back((Instant::now() + settings.retransmission_delay, index));

                    statistics.retransmissions.inc();

                    true
                } else {
                    false
                }
            }
            PaceOutTask::Send(destination, mut message) => {
                let index = *cursor;
                *cursor += 1;

                message.buffer[message.size] = 0; // First footer byte is 0: MESSAGE

                message.buffer[(message.size + 1)..(message.size + 9)]
                    .clone_from_slice((index as u64).to_le_bytes().as_slice());

                message.size += 9;

                let _ = route_out_inlet.send((destination, message.clone())).await;

                datagram_table.push(destination, message);
                retransmission_queue
                    .push_back((Instant::now() + settings.retransmission_delay, index));

                true
            }
        }
    }

    fn route_out(socket: Arc<UdpSocket>, mut route_out_outlet: DatagramOutlet) {
        loop {
            let (destination, message) = if let Some(datagram) = route_out_outlet.blocking_recv() {
                datagram
            } else {
                // `DatagramDispatcher` has dropped, shutdown
                return;
            };

            socket
                .send_to(&message.buffer[..message.size], destination)
                .unwrap(); // TODO: Determine if this can fail
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
}
