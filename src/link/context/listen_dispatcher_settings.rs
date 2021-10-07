#[derive(Debug, Clone)]
pub struct ListenDispatcherSettings {
    pub channel_capacity: usize,
}

impl Default for ListenDispatcherSettings {
    fn default() -> Self {
        ListenDispatcherSettings {
            channel_capacity: 32,
        }
    }
}
