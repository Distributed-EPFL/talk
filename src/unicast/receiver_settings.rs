#[derive(Debug, Clone)]
pub struct ReceiverSettings {
    pub message_channel_capacity: usize,
    pub response_channel_capacity: usize,
}

impl Default for ReceiverSettings {
    fn default() -> Self {
        ReceiverSettings {
            message_channel_capacity: 32768,
            response_channel_capacity: 64,
        }
    }
}
