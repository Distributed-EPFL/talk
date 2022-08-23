use crate::net::datagram_dispatcher::MAXIMUM_TRANSMISSION_UNIT;

#[derive(Clone)]
pub(in crate::net::datagram_dispatcher) struct Message {
    pub buffer: [u8; MAXIMUM_TRANSMISSION_UNIT],
    pub size: usize,
}
