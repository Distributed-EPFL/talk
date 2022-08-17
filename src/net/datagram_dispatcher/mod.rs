use doomstack::{here, Doom, ResultExt, Top};

use std::{
    io,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::Arc,
    time::Duration,
};

use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};

type DatagramInlet = MpscSender<(SocketAddr, Message)>;
type DatagramOutlet = MpscReceiver<(SocketAddr, Message)>;

const MAXIMUM_TRANSMISSION_UNIT: usize = 2048;

// TODO: Turn into settings
const PROCESS_TASKS: usize = 4;
const PROCESS_CHANNEL_CAPACITY: usize = 1024;

pub struct DatagramDispatcher {}

#[derive(Doom)]
pub enum DatagramDispatcherError {
    #[doom(description("Failed to bind address: {:?}", source))]
    #[doom(wrap(bind_failed))]
    BindFailed { source: io::Error },
}

struct Message {
    buffer: [u8; MAXIMUM_TRANSMISSION_UNIT],
    size: usize,
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

        {
            let socket = socket.clone();

            tokio::task::spawn_blocking(move || {
                DatagramDispatcher::route_in(socket, process_inlets)
            });
        }

        todo!()
    }

    fn route_in(socket: Arc<UdpSocket>, process_inlets: Vec<DatagramInlet>) {
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap(); // TODO: Determine if this call can fail

        for robin in 0.. {
            let mut buffer = [0u8; MAXIMUM_TRANSMISSION_UNIT];

            let (size, source) = match socket.recv_from(&mut buffer) {
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
}
