use crate::unicast::PushSettings;

#[derive(Debug, Clone, Default)]
pub struct BestEffortSettings {
    pub push_settings: PushSettings,
}
