use crate::net::datagram_dispatcher::UdpWrapSettings;

use doomstack::{here, Doom, ResultExt, Top};

use nix::sys::socket::{recvmmsg, sendmmsg, MsgFlags, RecvMmsgData, SendMmsgData, SockaddrStorage};

use socket2::{Domain, Socket, Type};

use std::{
    io::{IoSlice, IoSliceMut},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    os::unix::io::AsRawFd,
};

use tokio::{
    io::{self, Interest},
    net::{self, ToSocketAddrs, UdpSocket},
};

pub(in crate::net::datagram_dispatcher) struct UdpWrap {
    socket: UdpSocket,
    settings: UdpWrapSettings,
}

pub(in crate::net::datagram_dispatcher) struct ReceiveMultiple {
    buffer: Vec<u8>,
    maximum_transfer_unit: usize,
    messages: Vec<(SocketAddr, usize)>,
}

#[derive(Doom)]
pub(in crate::net::datagram_dispatcher) enum UdpWrapError {
    #[doom(description("Failed to lookup bind address: {:?}", source))]
    #[doom(wrap(lookup_failed))]
    LookupFailed { source: io::Error },
    #[doom(description("Bind address unknown"))]
    BindUnknown,
    #[doom(description("Failed to bind address: {:?}", source))]
    #[doom(wrap(bind_failed))]
    BindFailed { source: io::Error },
    #[doom(description("Socket error: {:?}", source))]
    #[doom(wrap(socket_error))]
    SocketError { source: io::Error },
    #[doom(description("Send failed: {:?}", source))]
    #[doom(wrap(send_failed))]
    SendFailed { source: io::Error },
}

impl UdpWrap {
    pub async fn bind<A>(
        address: A,
        settings: UdpWrapSettings,
    ) -> Result<UdpWrap, Top<UdpWrapError>>
    where
        A: ToSocketAddrs,
    {
        let address: SocketAddr = net::lookup_host(address)
            .await
            .map_err(UdpWrapError::lookup_failed)
            .map_err(UdpWrapError::into_top)
            .spot(here!())?
            .next()
            .ok_or(UdpWrapError::BindUnknown.into_top())
            .spot(here!())?;

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();

        socket.set_reuse_port(true).unwrap();
        socket.set_nonblocking(true).unwrap();

        socket
            .bind(&address.into())
            .map_err(UdpWrapError::bind_failed)
            .map_err(UdpWrapError::into_top)
            .spot(here!())?;

        let socket = UdpSocket::from_std(socket.into()).unwrap();

        Ok(UdpWrap { socket, settings })
    }

    pub async fn send_multiple<'m, I>(&self, messages: I) -> Result<usize, Top<UdpWrapError>>
    where
        I: IntoIterator<Item = (&'m SocketAddr, &'m [u8])>,
    {
        let data = messages
            .into_iter()
            .map(|(address, buffer)| {
                let address: SockaddrStorage = (*address).into();

                SendMmsgData {
                    iov: [IoSlice::new(buffer)],
                    cmsgs: &[],
                    addr: Some(address),
                    _lt: Default::default(),
                }
            })
            .collect::<Vec<SendMmsgData<_, _, _>>>();

        self.socket
            .writable()
            .await
            .map_err(UdpWrapError::socket_error)
            .map_err(UdpWrapError::into_top)
            .spot(here!())?;

        let sent = self
            .socket
            .try_io(Interest::WRITABLE, || {
                let descriptor = self.socket.as_raw_fd();

                let sizes =
                    sendmmsg(descriptor, &data, MsgFlags::MSG_DONTWAIT).map_err(|error| {
                        if error == nix::errno::Errno::EWOULDBLOCK {
                            return io::Error::new(
                                io::ErrorKind::WouldBlock,
                                "`sendmmsg` would block",
                            );
                        }

                        io::Error::new(io::ErrorKind::Other, error)
                    })?;

                let sent = sizes
                    .iter()
                    .position(|size| *size == 0)
                    .unwrap_or(sizes.len());

                Ok(sent)
            })
            .map_err(UdpWrapError::send_failed)
            .map_err(UdpWrapError::into_top)
            .spot(here!())?;

        Ok(sent)
    }

    pub async fn receive_multiple(&self) -> ReceiveMultiple {
        let mut buffer =
            vec![0u8; self.settings.maximum_transmission_unit * self.settings.receive_buffer_size];

        let mut data = buffer
            .chunks_exact_mut(self.settings.maximum_transmission_unit)
            .map(|chunk| RecvMmsgData {
                iov: [IoSliceMut::new(chunk)],
                cmsg_buffer: None,
            })
            .collect::<Vec<RecvMmsgData<_>>>();

        self.socket.readable().await.unwrap();

        let messages: Vec<_> = self
            .socket
            .try_io(Interest::READABLE, || {
                let descriptor = self.socket.as_raw_fd();

                let messages = recvmmsg(descriptor, &mut data, MsgFlags::MSG_DONTWAIT, None)
                    .map_err(|error| {
                        if error == nix::errno::Errno::EWOULDBLOCK {
                            return io::Error::new(
                                io::ErrorKind::WouldBlock,
                                "recvmmsg would block",
                            );
                        }
                        io::Error::new(io::ErrorKind::Other, error)
                    })?
                    .iter()
                    .map(|message| {
                        let sockaddr_storage: SockaddrStorage = message.address.unwrap();
                        let sockaddr_in = sockaddr_storage.as_sockaddr_in().unwrap();

                        let socketaddr_v4 =
                            SocketAddrV4::new(Ipv4Addr::from(sockaddr_in.ip()), sockaddr_in.port());

                        let address = SocketAddr::V4(socketaddr_v4);

                        (address, message.bytes)
                    })
                    .collect();

                Ok(messages)
            })
            .unwrap_or_default();

        ReceiveMultiple {
            buffer,
            maximum_transfer_unit: self.settings.maximum_transmission_unit,
            messages,
        }
    }
}

impl ReceiveMultiple {
    pub fn iter(&self) -> impl Iterator<Item = (&SocketAddr, &[u8])> {
        self.messages
            .iter()
            .zip(self.buffer.chunks_exact(self.maximum_transfer_unit))
            .map(|((address, size), buffer)| (address, &buffer[..*size]))
    }
}
