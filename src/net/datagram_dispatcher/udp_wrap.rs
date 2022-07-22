use doomstack::{here, Doom, ResultExt, Top};

use nix::sys::socket::{sendmmsg, MsgFlags, SendMmsgData, SockaddrStorage};

use socket2::{Domain, Socket, Type};

use std::{io::IoSlice, net::SocketAddr, os::unix::io::AsRawFd};

use tokio::{
    io::{self, Interest},
    net::{self, ToSocketAddrs, UdpSocket},
};

pub(in crate::net::datagram_dispatcher) struct UdpWrap {
    socket: UdpSocket,
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
    pub async fn bind<A>(address: A) -> Result<UdpWrap, Top<UdpWrapError>>
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

        Ok(UdpWrap { socket })
    }

    async fn send_multiple<'m, I>(&self, messages: I) -> Result<usize, Top<UdpWrapError>>
    where
        I: IntoIterator<Item = &'m (SocketAddr, &'m [u8])>,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn develop() {
        let wrap = UdpWrap::bind("0.0.0.0:0").await.unwrap();

        let buffer = [42u8; 288];
        let messages = vec![("127.0.0.1:1234".parse().unwrap(), &buffer[..]); 100000];

        wrap.send_multiple(&messages).await.unwrap();
    }
}
