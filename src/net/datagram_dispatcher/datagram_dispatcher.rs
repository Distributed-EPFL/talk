use crate::net::datagram_dispatcher::{DatagramReceiver, DatagramSender};

use doomstack::{here, Doom, ResultExt, Top};
use tokio::net::ToSocketAddrs;

pub struct DatagramDispatcher {
    sender: DatagramSender,
    receiver: DatagramReceiver,
}

#[derive(Doom)]
pub enum DatagramDispatcherError {
    #[doom(description("Failed to `bind` to the address provided"))]
    BindFailed,
}

impl DatagramDispatcher {
    pub async fn bind<A>(address: A) -> Result<DatagramDispatcher, Top<DatagramDispatcherError>>
    where
        A: ToSocketAddrs,
    {
        todo!()
    }
}
