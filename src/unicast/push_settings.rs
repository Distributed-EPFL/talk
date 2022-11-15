use crate::{
    time::{sleep_schedules::CappedExponential, SleepSchedule},
    unicast::Acknowledgement,
};
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone)]
pub struct PushSettings {
    pub stop_condition: Acknowledgement,
    pub retry_schedule: Arc<dyn SleepSchedule>,
}

#[derive(Debug, Clone)]
pub struct PartialPushSettings {
    pub retry_schedule: Arc<dyn SleepSchedule>,
}

impl PushSettings {
    pub fn compose(
        stop_condition: Acknowledgement,
        partial_push_settings: PartialPushSettings,
    ) -> Self {
        PushSettings {
            stop_condition,
            retry_schedule: partial_push_settings.retry_schedule,
        }
    }
}

impl Default for PushSettings {
    fn default() -> Self {
        PushSettings {
            stop_condition: Acknowledgement::Strong,
            retry_schedule: Arc::new(CappedExponential::new(
                Duration::from_secs(5),
                2.,
                Duration::from_secs(300),
            )),
        }
    }
}

impl Default for PartialPushSettings {
    fn default() -> Self {
        let push_settings = PushSettings::default();

        PartialPushSettings {
            retry_schedule: push_settings.retry_schedule,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::time::sleep_schedules::Constant;

    impl PushSettings {
        pub fn strong_constant() -> Self {
            PushSettings {
                stop_condition: Acknowledgement::Strong,
                retry_schedule: Arc::new(Constant::new(Duration::from_millis(100))),
            }
        }
    }
}
