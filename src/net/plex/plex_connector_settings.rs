use crate::net::plex::MultiplexSettings;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PlexConnectorSettings {
    pub connections_per_remote: usize,
    pub keep_alive_interval: Duration,
    pub multiplex_settings: MultiplexSettings,
}

impl Default for PlexConnectorSettings {
    fn default() -> Self {
        PlexConnectorSettings {
            connections_per_remote: 10,
            keep_alive_interval: Duration::from_secs(20),
            multiplex_settings: Default::default(),
        }
    }
}
