use crate::net::datagram_dispatcher::Message;

use std::collections::VecDeque;

pub(in crate::net::datagram_dispatcher) struct MessageTable {
    messages: VecDeque<Message>,
    offset: usize,
}
