use atomic_counter::RelaxedCounter;

pub(in crate::net::datagram_dispatcher) struct Statistics {
    pub retransmissions: RelaxedCounter,
}
