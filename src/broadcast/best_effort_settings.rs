use crate::unicast::PushSettings;

#[derive(Debug, Clone, Default)]
pub struct BestEffortSettings {
    pub push_settings: PushSettings,
}

#[cfg(test)]
mod test {
    use super::*;

    impl BestEffortSettings {
        pub fn strong_constant() -> Self {
            BestEffortSettings {
                push_settings: PushSettings::strong_constant(),
            }
        }
    }
}
