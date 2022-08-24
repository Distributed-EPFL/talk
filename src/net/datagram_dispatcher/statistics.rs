use atomic_counter::RelaxedCounter;

use std::sync::Mutex;

pub(in crate::net::datagram_dispatcher) struct Statistics {
    pub packets_sent: RelaxedCounter,
    pub packets_received: RelaxedCounter,
    pub message_packets_processed: RelaxedCounter,
    pub acknowledgement_packets_processed: RelaxedCounter,
    pub retransmissions: RelaxedCounter,
    pub pace_out_chokes: RelaxedCounter,
    pub process_in_drops: RelaxedCounter,
    pub route_out_drops: RelaxedCounter,
    pub retransmission_queue_len: Mutex<usize>,
}
