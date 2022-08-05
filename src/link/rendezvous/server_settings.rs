use crate::net::traits::ConnectSettings;

#[derive(Debug, Clone)]
pub struct ServerSettings {
    pub shard_sizes: Vec<usize>,
    pub connect: ConnectSettings,
}

impl Default for ServerSettings {
    fn default() -> Self {
        ServerSettings {
            shard_sizes: vec![4],
            connect: ConnectSettings::default(),
        }
    }
}
