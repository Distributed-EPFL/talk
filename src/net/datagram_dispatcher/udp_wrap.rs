use doomstack::{here, Doom, ResultExt, Top};

use socket2::{Domain, Socket, Type};

use std::net::SocketAddr;

use tokio::{
    io,
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
}
