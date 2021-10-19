use crate::unicast::SenderSettings;

#[derive(Debug, Clone)]
pub(in crate::unicast) struct CasterSettings {
    pub request_channel_capacity: usize,
}

impl CasterSettings {
    pub fn from_sender_settings(settings: &SenderSettings) -> Self {
        CasterSettings {
            request_channel_capacity: settings.request_channel_capacity,
        }
    }
}

impl Default for CasterSettings {
    fn default() -> Self {
        CasterSettings::from_sender_settings(&SenderSettings::default())
    }
}
