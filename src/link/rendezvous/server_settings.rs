#[derive(Debug, Clone)]
pub struct ServerSettings {
    pub shard_sizes: Vec<usize>,
}

impl Default for ServerSettings {
    fn default() -> Self {
        ServerSettings {
            shard_sizes: vec![4],
        }
    }
}
