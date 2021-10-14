use crate::net::ConnectionSettings;

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ReceiverSettings {
    pub receive_timeout: Option<Duration>,
}

impl Default for ReceiverSettings {
    fn default() -> Self {
        ReceiverSettings {
            receive_timeout: ConnectionSettings::default().receive_timeout,
        }
    }
}
