#[derive(Clone, Debug)]
pub(in crate::net::datagram_dispatcher) struct UdpWrapSettings {
    pub maximum_transmission_unit: usize,
    pub receive_buffer_size: usize,
}

impl Default for UdpWrapSettings {
    fn default() -> UdpWrapSettings {
        UdpWrapSettings {
            maximum_transmission_unit: 2048,
            receive_buffer_size: 128,
        }
    }
}
