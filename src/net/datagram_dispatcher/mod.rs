mod datagram_dispatcher;
mod datagram_dispatcher_settings;
mod datagram_table;
mod message;

pub use datagram_dispatcher::{DatagramDispatcher, DatagramDispatcherError};
pub use datagram_dispatcher_settings::DatagramDispatcherSettings;

use datagram_table::DatagramTable;
use message::Message;

const MAXIMUM_TRANSMISSION_UNIT: usize = 2048;
