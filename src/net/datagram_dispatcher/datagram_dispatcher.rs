use crate::{
    net::{
        datagram_dispatcher::{DatagramTable, Message, MAXIMUM_TRANSMISSION_UNIT},
        Message as NetMessage,
    },
    sync::fuse::{Fuse, Relay},
};

use doomstack::{here, Doom, ResultExt, Top};

use rand::prelude::*;

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

// TODO: Turn into settings
const RETRANSMISSION_DELAY: Duration = Duration::from_millis(100);

const RECEIVE_CHANNEL_CAPACITY: usize = 1024;

const PROCESS_IN_TASKS: usize = 4;
const PROCESS_IN_CHANNEL_CAPACITY: usize = 1024;

const PROCESS_OUT_TASKS: usize = 4;
const PROCESS_OUT_CHANNEL_CAPACITY: usize = 1024;

const ROUTE_OUT_RATE: f64 = 65536.;

const ROUTE_OUT_WINDOW_MIN: Duration = Duration::from_millis(10);
const ROUTE_OUT_WINDOW_MAX: Duration = Duration::from_millis(20);

const ROUTE_OUT_DATAGRAM_CHANNEL_CAPACITY: usize = 4096;
const ROUTE_OUT_ACKNOWLEDGEMENT_CHANNEL_CAPACITY: usize = 4096;
const ROUTE_OUT_COMPLETION_CHANNEL_CAPACITY: usize = 4096;

pub struct DatagramDispatcher<S: NetMessage, R: NetMessage> {
    receive_outlet: MessageOutlet<R>,
    process_out_inlets: Vec<MessageInlet<S>>,
    _fuse: Fuse,
}

#[derive(Doom)]
pub enum DatagramDispatcherError {
    #[doom(description("Failed to bind address: {:?}", source))]
    #[doom(wrap(bind_failed))]
    BindFailed { source: io::Error },
}

enum RouteOutTask {
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
    pub fn bind<A>(address: A) -> Result<Self, Top<DatagramDispatcherError>>
    where
        A: ToSocketAddrs,
    {
        let socket = UdpSocket::bind(address)
            .map_err(DatagramDispatcherError::bind_failed)
            .map_err(DatagramDispatcherError::into_top)
            .spot(here!())?;

        let socket = Arc::new(socket);

        let (receive_inlet, receive_outlet) = mpsc::channel(RECEIVE_CHANNEL_CAPACITY);

        let mut process_in_inlets = Vec::new();
        let mut process_in_outlets = Vec::new();

        for _ in 0..PROCESS_IN_TASKS {
            let (process_in_inlet, process_in_outlet) = mpsc::channel(PROCESS_IN_CHANNEL_CAPACITY);

            process_in_inlets.push(process_in_inlet);
            process_in_outlets.push(process_in_outlet);
        }

        let mut process_out_inlets = Vec::new();
        let mut process_out_outlets = Vec::new();

        for _ in 0..PROCESS_OUT_TASKS {
            let (process_out_inlet, process_out_outlet) =
                mpsc::channel(PROCESS_OUT_CHANNEL_CAPACITY);

            process_out_inlets.push(process_out_inlet);
            process_out_outlets.push(process_out_outlet);
        }

        let (_route_out_datagram_inlet, route_out_datagram_outlet) =
            mpsc::channel(ROUTE_OUT_DATAGRAM_CHANNEL_CAPACITY);

        let (route_out_acknowledgement_inlet, route_out_acknowledgement_outlet) =
            mpsc::channel(ROUTE_OUT_ACKNOWLEDGEMENT_CHANNEL_CAPACITY);

        let (route_out_completion_inlet, route_out_completion_outlet) =
            mpsc::channel(ROUTE_OUT_COMPLETION_CHANNEL_CAPACITY);

        let fuse = Fuse::new();

        {
            let socket = socket.clone();
            let relay = fuse.relay();

            task::spawn_blocking(move || {
                DatagramDispatcher::<S, R>::route_in(socket, process_in_inlets, relay)
            });
        }

        for process_in_outlet in process_in_outlets {
            fuse.spawn(DatagramDispatcher::<S, R>::process_in(
                process_in_outlet,
                receive_inlet.clone(),
                route_out_acknowledgement_inlet.clone(),
                route_out_completion_inlet.clone(),
            ));
        }

        for process_out_outlet in process_out_outlets {
            fuse.spawn(DatagramDispatcher::<S, R>::process_out(
                process_out_outlet,
                _route_out_datagram_inlet.clone(),
            ));
        }

        {
            let socket = socket.clone();

            fuse.spawn(DatagramDispatcher::<S, R>::route_out(
                socket,
                route_out_datagram_outlet,
                route_out_acknowledgement_outlet,
                route_out_completion_outlet,
            ));
        }

        Ok(DatagramDispatcher {
            receive_outlet,
            process_out_inlets,
            _fuse: fuse,
        })
    }

    pub async fn send(&self, destination: SocketAddr, payload: S) {
        let _ = self
            .process_out_inlets
            .choose(&mut thread_rng())
            .unwrap()
            .send((destination, payload));
    }

    pub async fn receive(&mut self) -> (SocketAddr, R) {
        self.receive_outlet.recv().await.unwrap()
    }

    fn route_in(socket: Arc<UdpSocket>, process_inlets: Vec<DatagramInlet>, relay: Relay) {
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

            let _ = process_inlets
                .get(robin % PROCESS_IN_TASKS)
                .unwrap()
                .try_send((source, message)); // TODO: Log warning in case of `Err`
        }
    }

    async fn process_in(
        mut process_in_outlet: DatagramOutlet,
        receive_inlet: MessageInlet<R>,
        route_out_acknowledgement_inlet: AcknowledgementInlet,
        route_out_completion_inlet: CompletionInlet,
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

                let _ = route_out_acknowledgement_inlet.send((source, index)).await;

                if let Ok(payload) = bincode::deserialize::<R>(payload) {
                    let _ = receive_inlet.send((source, payload)).await;
                }
            } else if footer[1] == 1 {
                // First footer byte is 1: ACKNOWLEDGEMENT

                let _ = route_out_completion_inlet.send(index).await;
            }
        }
    }

    async fn process_out(
        mut process_out_outlet: MessageOutlet<S>,
        route_out_datagram_inlet: DatagramInlet,
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
                let _ = route_out_datagram_inlet.send((destination, message)).await;
            }
        }
    }

    async fn route_out(
        socket: Arc<UdpSocket>,
        mut route_out_datagram_outlet: DatagramOutlet,
        mut route_out_acknowledgement_outlet: AcknowledgementOutlet,
        mut route_out_completion_outlet: CompletionOutlet,
    ) {
        let mut cursor = 0;

        let mut datagram_table = DatagramTable::new();
        let mut retransmission_queue = VecDeque::new();

        loop {
            let start = Instant::now();
            time::sleep(ROUTE_OUT_WINDOW_MIN).await;

            let task = if let Some(task) = DatagramDispatcher::<S, R>::wait_route_out_task(
                &mut route_out_datagram_outlet,
                &mut route_out_acknowledgement_outlet,
                &mut route_out_completion_outlet,
                &mut retransmission_queue,
            )
            .await
            {
                task
            } else {
                // `DatagramDispatcher` has dropped, shutdown
                return;
            };

            let slept = start.elapsed();

            let target =
                (cmp::min(slept, ROUTE_OUT_WINDOW_MAX).as_secs_f64() * ROUTE_OUT_RATE) as usize;

            let mut fulfilled = 0;

            if DatagramDispatcher::<S, R>::fulfill_route_out_task(
                socket.as_ref(),
                &mut cursor,
                &mut datagram_table,
                &mut retransmission_queue,
                task,
            )
            .await
            {
                fulfilled += 1;
            }

            while fulfilled < target {
                let task = if let Some(task) = DatagramDispatcher::<S, R>::next_route_out_task(
                    &mut route_out_datagram_outlet,
                    &mut route_out_acknowledgement_outlet,
                    &mut route_out_completion_outlet,
                    &mut retransmission_queue,
                ) {
                    task
                } else {
                    // No more tasks to fulfill on the fly
                    break;
                };

                if DatagramDispatcher::<S, R>::fulfill_route_out_task(
                    socket.as_ref(),
                    &mut cursor,
                    &mut datagram_table,
                    &mut retransmission_queue,
                    task,
                )
                .await
                {
                    fulfilled += 1;
                }
            }
        }
    }

    async fn wait_route_out_task(
        route_out_datagram_outlet: &mut DatagramOutlet,
        route_out_acknowledgement_outlet: &mut AcknowledgementOutlet,
        route_out_completion_outlet: &mut CompletionOutlet,
        retransmission_queue: &mut VecDeque<(Instant, usize)>,
    ) -> Option<RouteOutTask> {
        let next_retransmission = retransmission_queue
            .front()
            .map(|(next_retransmission, _)| *next_retransmission);

        tokio::select! {
            biased;

            completion = route_out_completion_outlet.recv() => {
                completion.map(|index| RouteOutTask::Complete(index))
            }
            acknowledgement = route_out_acknowledgement_outlet.recv() => {
                acknowledgement.map(|(destination, index)| RouteOutTask::Acknowledge(destination, index))
            },
            _ = time::sleep(next_retransmission.unwrap() - Instant::now()), if next_retransmission.is_some() => {
                let (_, index) = retransmission_queue.pop_front().unwrap();
                Some(RouteOutTask::Resend(index))
            },
            datagram = route_out_datagram_outlet.recv() => {
                datagram.map(|(destination, message)| RouteOutTask::Send(destination, message))
            }
        }
    }

    fn next_route_out_task(
        route_out_datagram_outlet: &mut DatagramOutlet,
        route_out_acknowledgement_outlet: &mut AcknowledgementOutlet,
        route_out_completion_outlet: &mut CompletionOutlet,
        retransmission_queue: &mut VecDeque<(Instant, usize)>,
    ) -> Option<RouteOutTask> {
        if let Ok(index) = route_out_completion_outlet.try_recv() {
            return Some(RouteOutTask::Complete(index));
        }

        if let Ok((destination, index)) = route_out_acknowledgement_outlet.try_recv() {
            return Some(RouteOutTask::Acknowledge(destination, index));
        }

        if let Some((next_retransmission, _)) = retransmission_queue.front() {
            if *next_retransmission > Instant::now() {
                let (_, index) = retransmission_queue.pop_front().unwrap();
                return Some(RouteOutTask::Resend(index));
            }
        }

        if let Ok((destination, message)) = route_out_datagram_outlet.try_recv() {
            return Some(RouteOutTask::Send(destination, message));
        }

        None
    }

    async fn fulfill_route_out_task(
        socket: &UdpSocket,
        cursor: &mut usize,
        datagram_table: &mut DatagramTable,
        retransmission_queue: &mut VecDeque<(Instant, usize)>,
        task: RouteOutTask,
    ) -> bool {
        match task {
            RouteOutTask::Complete(index) => {
                datagram_table.remove(index);
                false
            }
            RouteOutTask::Acknowledge(destination, index) => {
                let mut buffer = [1u8; 9]; // First footer byte is 1: ACKNOWLEDGEMENT (note that the following bytes are overwritten)
                buffer[1..].clone_from_slice((index as u64).to_le_bytes().as_slice());
                socket.send_to(buffer.as_slice(), destination).unwrap(); // TODO: Determine if this can fail

                true
            }
            RouteOutTask::Resend(index) => {
                if let Some((destination, message)) = datagram_table.get(index) {
                    socket
                        .send_to(&message.buffer[..message.size], *destination)
                        .unwrap(); // TODO: Determine if this can fail

                    retransmission_queue.push_back((Instant::now() + RETRANSMISSION_DELAY, index));

                    true
                } else {
                    false
                }
            }
            RouteOutTask::Send(destination, mut message) => {
                let index = *cursor;
                *cursor += 1;

                message.buffer[message.size] = 0; // First footer byte is 0: MESSAGE

                message.buffer[(message.size + 1)..(message.size + 9)]
                    .clone_from_slice((index as u64).to_le_bytes().as_slice());

                message.size += 9;

                socket
                    .send_to(&message.buffer[..message.size], destination)
                    .unwrap(); // TODO: Determine if this can fail

                datagram_table.push(destination, message);
                retransmission_queue.push_back((Instant::now() + RETRANSMISSION_DELAY, index));

                true
            }
        }
    }
}
