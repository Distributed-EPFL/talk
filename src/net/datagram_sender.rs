use crate::net::Message;

use socket2::{Domain, Socket, Type};

use std::{marker::PhantomData, net::SocketAddr};

use tokio::net::{self, ToSocketAddrs, UdpSocket};

pub struct DatagramSender<M: Message> {
    socket: UdpSocket,
    _phantom: PhantomData<M>,
}

impl<M> DatagramSender<M>
where
    M: Message,
{
    pub async fn bind<A>(address: A) -> DatagramSender<M>
    where
        A: ToSocketAddrs,
    {
        // TODO: Manage errors
        let address: SocketAddr = net::lookup_host(address).await.unwrap().next().unwrap();

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();

        socket.set_reuse_port(true).unwrap();
        socket.set_nonblocking(true).unwrap();

        socket.bind(&address.into()).unwrap();

        let socket = UdpSocket::from_std(socket.into()).unwrap();

        DatagramSender {
            socket,
            _phantom: PhantomData,
        }
    }

    pub async fn send<A>(&self, message: &M, destination: A)
    where
        A: ToSocketAddrs,
    {
        // TODO: Manage errors
        let buffer = bincode::serialize(&message).unwrap();

        self.socket
            .send_to(buffer.as_slice(), destination)
            .await
            .unwrap();
    }
}
