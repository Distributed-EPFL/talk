pub struct PlexConnectorSettings {
    pub max_connections_per_identity: usize,
}

impl Default for PlexConnectorSettings {
    fn default() -> Self {
        PlexConnectorSettings {
            max_connections_per_identity: 10,
        }
    }
}
