use atomic_counter::RelaxedCounter;

pub(in crate::net::datagram_dispatcher) struct Statistics {
    pub retransmissions: RelaxedCounter,
    pub process_in_drops: RelaxedCounter,
}
