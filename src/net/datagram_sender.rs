use crate::net::Message;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use socket2::{Domain, Socket, Type};

use std::{
    cmp,
    marker::PhantomData,
    net::SocketAddr,
    time::{Duration, Instant},
};

use tokio::{
    net::{self, ToSocketAddrs, UdpSocket},
    time,
};

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

    pub async fn send_multiple<A>(&self, messages: &[(A, M)], duration: Duration)
    where
        A: Sync + ToSocketAddrs,
    {
        let messages = messages
            .par_iter()
            .map(|(destination, message)| (destination, bincode::serialize(&message).unwrap()))
            .collect::<Vec<_>>();

        let start = Instant::now();
        let mut sent = 0;

        while sent < messages.len() {
            let elapsed = start.elapsed().as_secs_f64() / duration.as_secs_f64();

            let target = (elapsed * (messages.len() as f64)) as usize;
            let target = cmp::min(target, messages.len());

            for (destination, message) in &messages[sent..target] {
                self.socket
                    .send_to(message.as_slice(), destination)
                    .await
                    .unwrap();
            }

            sent = target;

            // TODO: Add settings
            time::sleep(Duration::from_millis(10)).await;
        }
    }
}
