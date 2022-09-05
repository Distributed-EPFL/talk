mod datagram_dispatcher;
mod datagram_dispatcher_settings;
mod datagram_receiver;
mod datagram_sender;
mod datagram_table;
mod message;
mod statistics;

pub use datagram_dispatcher::{
    DatagramDispatcher, DatagramDispatcherError, DatagramDispatcherMode,
};
pub use datagram_dispatcher_settings::DatagramDispatcherSettings;
pub use datagram_receiver::DatagramReceiver;
pub use datagram_sender::DatagramSender;

use datagram_table::DatagramTable;
use message::Message;
use statistics::Statistics;

const MAXIMUM_TRANSMISSION_UNIT: usize = 2048;
