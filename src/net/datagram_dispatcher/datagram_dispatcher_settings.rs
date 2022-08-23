use std::time::Duration;

#[derive(Debug, Clone)]
pub struct DatagramDispatcherSettings {
    pub retransmission_delay: Duration,
    pub maximum_packet_rate: f64,

    pub minimum_rate_window: Duration,
    pub maximum_rate_window: Duration,

    pub process_in_tasks: usize,
    pub process_out_tasks: usize,

    pub receive_channel_capacity: usize,
    pub process_in_channel_capacity: usize,
    pub process_out_channel_capacity: usize,
    pub pace_out_datagram_channel_capacity: usize,
    pub pace_out_acknowledgement_channel_capacity: usize,
    pub pace_out_completion_channel_capacity: usize,
}

impl Default for DatagramDispatcherSettings {
    fn default() -> Self {
        DatagramDispatcherSettings {
            retransmission_delay: Duration::from_millis(100),
            maximum_packet_rate: 65536.,
            minimum_rate_window: Duration::from_millis(10),
            maximum_rate_window: Duration::from_millis(20),
            process_in_tasks: 4,
            process_out_tasks: 4,
            receive_channel_capacity: 4096,
            process_in_channel_capacity: 1024,
            process_out_channel_capacity: 1024,
            pace_out_datagram_channel_capacity: 4096,
            pace_out_acknowledgement_channel_capacity: 4096,
            pace_out_completion_channel_capacity: 4096,
        }
    }
}
