use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SenderSettings {
    pub message_channel_capacity: usize,
    pub link_timeout: Duration,
    pub keepalive_interval: Duration,
}

impl Default for SenderSettings {
    fn default() -> Self {
        SenderSettings {
            message_channel_capacity: 32768,
            link_timeout: Duration::from_secs(1800),
            keepalive_interval: Duration::from_secs(10),
        }
    }
}
