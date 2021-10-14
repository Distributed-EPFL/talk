use crate::net::ConnectionSettings;

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SenderSettings {
    pub send_timeout: Option<Duration>,
}

impl Default for SenderSettings {
    fn default() -> Self {
        SenderSettings {
            send_timeout: ConnectionSettings::default().send_timeout,
        }
    }
}
