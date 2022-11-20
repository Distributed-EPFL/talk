use crate::net::plex::PlexSettings;

#[derive(Debug, Clone)]
pub struct MultiplexSettings {
    pub run_plex_channel_capacity: usize,
    pub run_route_in_channel_capacity: usize,
    pub route_out_channel_capacity: usize,
    pub accept_channel_capacity: usize,
    pub plex_settings: PlexSettings,
}

impl Default for MultiplexSettings {
    fn default() -> Self {
        Self {
            run_plex_channel_capacity: 128,
            run_route_in_channel_capacity: 128,
            route_out_channel_capacity: 128,
            accept_channel_capacity: 128,
            plex_settings: Default::default(),
        }
    }
}
