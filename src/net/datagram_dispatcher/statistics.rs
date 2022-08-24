use atomic_counter::RelaxedCounter;

pub(in crate::net::datagram_dispatcher) struct Statistics {
    pub packets_sent: RelaxedCounter,
    pub packets_received: RelaxedCounter,
    pub retransmissions: RelaxedCounter,
    pub process_in_drops: RelaxedCounter,
    pub route_out_drops: RelaxedCounter,
}
