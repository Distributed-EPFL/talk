use crate::link::rendezvous::ClientSettings;

#[derive(Debug, Clone, Default)]
pub struct ConnectorSettings {
    pub client_settings: ClientSettings,
}
