use std::time::Duration;

#[derive(Debug, Clone)]
pub struct DatagramDispatcherSettings {
    pub retransmission_delay: Duration,
    pub maximum_packet_rate: f64,

    pub minimum_rate_window: Duration,
    pub maximum_rate_window: Duration,

    pub process_in_tasks: usize,
    pub process_out_tasks: usize,
    pub route_out_tasks: usize,

    pub receive_channel_capacity: usize,
    pub process_in_channel_capacity: usize,
    pub process_out_channel_capacity: usize,
    pub pace_out_datagram_channel_capacity: usize,
    pub pace_out_acknowledgement_channel_capacity: usize,
    pub pace_out_completion_channel_capacity: usize,
    pub route_out_channel_capacity: usize,

    pub route_out_batch_size: usize,
    pub route_in_batch_size: usize,

    pub pace_interval: Duration,
}

impl Default for DatagramDispatcherSettings {
    fn default() -> Self {
        DatagramDispatcherSettings {
            retransmission_delay: Duration::from_millis(100),
            maximum_packet_rate: 65536.,
            minimum_rate_window: Duration::from_millis(1),
            maximum_rate_window: Duration::from_millis(2),
            process_in_tasks: 8,
            process_out_tasks: 4,
            route_out_tasks: 2,
            receive_channel_capacity: 16384,
            process_in_channel_capacity: 4096,
            process_out_channel_capacity: 4096,
            pace_out_datagram_channel_capacity: 16384,
            pace_out_acknowledgement_channel_capacity: 16384,
            pace_out_completion_channel_capacity: 16384,
            route_out_channel_capacity: 16384,
            route_out_batch_size: 128,
            route_in_batch_size: 128,
            pace_interval: Duration::from_millis(10),
        }
    }
}
