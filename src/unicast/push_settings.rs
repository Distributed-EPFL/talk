use crate::{
    time::{sleep_schedules::CappedExponential, SleepSchedule},
    unicast::Acknowledgement,
};

use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PushSettings {
    pub stop_condition: Acknowledgement,
    pub retry_schedule: Arc<dyn SleepSchedule>,
}

impl Default for PushSettings {
    fn default() -> Self {
        PushSettings {
            stop_condition: Acknowledgement::Weak,
            retry_schedule: Arc::new(CappedExponential::new(
                Duration::from_secs(5),
                2.,
                Duration::from_secs(300),
            )),
        }
    }
}
