#[derive(Debug, Clone)]
pub struct PlexSettings {
    pub receive_channel_capacity: usize,
}

impl Default for PlexSettings {
    fn default() -> Self {
        PlexSettings {
            receive_channel_capacity: 128,
        }
    }
}
