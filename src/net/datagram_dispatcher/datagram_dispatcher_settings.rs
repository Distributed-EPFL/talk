#[derive(Debug, Clone)]
pub struct DatagramDispatcherSettings {
    pub workers: usize,
}

impl Default for DatagramDispatcherSettings {
    fn default() -> Self {
        DatagramDispatcherSettings { workers: 32 }
    }
}
