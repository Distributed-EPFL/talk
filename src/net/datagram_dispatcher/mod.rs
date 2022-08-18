mod datagram_dispatcher;
mod message;
mod message_table;

pub use datagram_dispatcher::{DatagramDispatcher, DatagramDispatcherError};

use message::Message;
use message_table::MessageTable;

const MAXIMUM_TRANSMISSION_UNIT: usize = 2048;
