use std::time::Duration;

#[derive(Debug, Clone)]
pub struct DatagramDispatcherSettings {
    pub workers: usize,
    pub retransmission_delay: Duration,
    pub retransmission_interval: Duration,

    pub receiver_channel_capacity: usize,
    pub route_out_channels_capacity: usize,
    pub acknowledgements_channels_capacity: usize,
    pub process_channels_capacity: usize,
}

impl Default for DatagramDispatcherSettings {
    fn default() -> Self {
        DatagramDispatcherSettings {
            workers: 32,
            retransmission_delay: Duration::from_millis(100),
            retransmission_interval: Duration::from_millis(10),

            receiver_channel_capacity: 1024,
            route_out_channels_capacity: 1024,
            acknowledgements_channels_capacity: 1024,
            process_channels_capacity: 1024,
        }
    }
}
