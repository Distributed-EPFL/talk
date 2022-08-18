mod datagram_dispatcher;
mod datagram_table;
mod message;

pub use datagram_dispatcher::{DatagramDispatcher, DatagramDispatcherError};

use datagram_table::DatagramTable;
use message::Message;

const MAXIMUM_TRANSMISSION_UNIT: usize = 2048;
