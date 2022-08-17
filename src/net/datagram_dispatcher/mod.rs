use doomstack::{here, Doom, ResultExt, Top};

use std::{
    io,
    net::{ToSocketAddrs, UdpSocket},
    sync::Arc,
};

pub struct DatagramDispatcher {}

#[derive(Doom)]
pub enum DatagramDispatcherError {
    #[doom(description("Failed to bind address: {:?}", source))]
    #[doom(wrap(bind_failed))]
    BindFailed { source: io::Error },
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

        {
            let socket = socket.clone();
            tokio::task::spawn_blocking(move || DatagramDispatcher::receive(socket));
        }

        todo!()
    }

    fn receive(socket: Arc<UdpSocket>) {}
}
