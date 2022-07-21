use crate::{net::Message, sync::fuse::Fuse};

use socket2::{Domain, Socket, Type};

use std::{iter, net::SocketAddr};

use tokio::{
    net::{self, ToSocketAddrs, UdpSocket},
    sync::mpsc::{self, Receiver, Sender},
};

pub struct DatagramReceiver<M: Message> {
    message_outlet: Receiver<(SocketAddr, M)>,
    _fuse: Fuse,
}

impl<M> DatagramReceiver<M>
where
    M: Message,
{
    pub async fn bind<A>(address: A) -> DatagramReceiver<M>
    where
        A: ToSocketAddrs,
    {
        // TODO: Manage errors
        let address: SocketAddr = net::lookup_host(address).await.unwrap().next().unwrap();

        let sockets = iter::repeat_with(|| {
            let socket = Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();

            socket.set_reuse_port(true).unwrap();
            socket.set_nonblocking(true).unwrap();

            socket.bind(&address.into()).unwrap();

            UdpSocket::from_std(socket.into()).unwrap()
        });

        // TODO: Add settings
        let (message_inlet, message_outlet) = mpsc::channel(1024);

        let fuse = Fuse::new();

        for socket in sockets {
            let message_inlet = message_inlet.clone();

            fuse.spawn(async move {
                let _ = DatagramReceiver::<M>::listen(socket, message_inlet).await;
            });
        }

        DatagramReceiver {
            message_outlet,
            _fuse: fuse,
        }
    }

    pub async fn receive(&mut self) -> (SocketAddr, M) {
        // The `receive` task(s) hold the corresponding `Sender`
        // for as long as the `DatagramReceiver` exists. As a
        // result, `recv()` is guaranteed to be `Ok`.
        self.message_outlet.recv().await.unwrap()
    }

    async fn listen(socket: UdpSocket, message_inlet: Sender<(SocketAddr, M)>) {
        let mut buffer = [0u8; 2048];

        loop {
            if let Ok((length, source)) = socket.recv_from(&mut buffer).await {
                if let Ok(message) = bincode::deserialize::<M>(&buffer[..length]) {
                    let _ = message_inlet.send((source, message));
                }
            }
        }
    }
}
