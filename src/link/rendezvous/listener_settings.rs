use crate::link::rendezvous::ClientSettings;

#[derive(Debug, Clone)]
pub struct ListenerSettings {
    pub client_settings: ClientSettings,
    pub channel_capacity: usize,
}

impl Default for ListenerSettings {
    fn default() -> Self {
        ListenerSettings {
            client_settings: ClientSettings::default(),
            channel_capacity: 32,
        }
    }
}
