use crate::{
    net::datagram_dispatcher::{Message, MessageTable, MAXIMUM_TRANSMISSION_UNIT},
    sync::fuse::{Fuse, Relay},
};

use doomstack::{here, Doom, ResultExt, Top};

use std::{
    collections::VecDeque,
    io,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{
    sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender},
    task,
};

type DatagramInlet = MpscSender<(SocketAddr, Message)>;
type DatagramOutlet = MpscReceiver<(SocketAddr, Message)>;

type CommandOutlet = MpscReceiver<Command>;

// TODO: Turn into settings
const PROCESS_TASKS: usize = 4;
const PROCESS_CHANNEL_CAPACITY: usize = 1024;
const ROUTE_OUT_CHANNEL_CAPACITY: usize = 4096;

pub struct DatagramDispatcher {
    _fuse: Fuse,
}

#[derive(Doom)]
pub enum DatagramDispatcherError {
    #[doom(description("Failed to bind address: {:?}", source))]
    #[doom(wrap(bind_failed))]
    BindFailed { source: io::Error },
}

enum Command {
    Send(Message),
    Acknowledge(usize),
}

impl DatagramDispatcher {
    pub fn bind<A>(address: A) -> Result<DatagramDispatcher, Top<DatagramDispatcherError>>
    where
        A: ToSocketAddrs,
    {
        let socket = UdpSocket::bind(address)
            .map_err(DatagramDispatcherError::bind_failed)
            .map_err(DatagramDispatcherError::into_top)
            .spot(here!())?;

        let socket = Arc::new(socket);

        let mut process_inlets = Vec::new();
        let mut process_outlets = Vec::new();

        for _ in 0..PROCESS_TASKS {
            let (process_inlet, process_outlet) = mpsc::channel(PROCESS_CHANNEL_CAPACITY);

            process_inlets.push(process_inlet);
            process_outlets.push(process_outlet);
        }

        let (route_out_inlet, route_out_outlet) = mpsc::channel(ROUTE_OUT_CHANNEL_CAPACITY);

        let fuse = Fuse::new();

        {
            let socket = socket.clone();
            let relay = fuse.relay();

            task::spawn_blocking(move || {
                DatagramDispatcher::route_in(socket, process_inlets, relay)
            });
        }

        for process_outlet in process_outlets {
            fuse.spawn(DatagramDispatcher::process(process_outlet));
        }

        {
            let socket = socket.clone();

            task::spawn_blocking(move || {
                DatagramDispatcher::route_out(socket, route_out_outlet);
            });
        }

        Ok(DatagramDispatcher { _fuse: fuse })
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
                .get(robin % PROCESS_TASKS)
                .unwrap()
                .try_send((source, message)); // TODO: Log warning in case of `Err`
        }
    }

    async fn process(mut process_outlet: DatagramOutlet) {
        loop {
            let (_source, _message) = if let Some(datagram) = process_outlet.recv().await {
                datagram
            } else {
                // `DatagramDispatcher` has dropped, shutdown
                return;
            };
        }
    }

    fn route_out(socket: Arc<UdpSocket>, route_out_outlet: CommandOutlet) {
        let mut message_table = MessageTable::new();
        let mut retransmission_queue: VecDeque<(Instant, usize)> = VecDeque::new();
    }
}
