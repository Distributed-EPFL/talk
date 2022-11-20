use crate::net::plex::MultiplexSettings;

#[derive(Debug, Clone)]
pub struct PlexListenerSettings {
    pub accept_channel_capacity: usize,
    pub multiplex_settings: MultiplexSettings,
}

impl Default for PlexListenerSettings {
    fn default() -> Self {
        PlexListenerSettings {
            accept_channel_capacity: 128,
            multiplex_settings: Default::default(),
        }
    }
}
