use atomic_counter::RelaxedCounter;

#[derive(Debug)]
pub struct Statistics {
    pub packets_sent: RelaxedCounter,
    pub packets_received: RelaxedCounter,
    pub message_packets_processed: RelaxedCounter,
    pub acknowledgement_packets_processed: RelaxedCounter,
    pub retransmissions: RelaxedCounter,
    pub pace_out_chokes: RelaxedCounter,
    pub process_in_drops: RelaxedCounter,
    pub route_out_drops: RelaxedCounter,
}
