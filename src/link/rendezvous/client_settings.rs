use crate::time::{sleep_schedules::CappedExponential, SleepSchedule};

use std::time::Duration;

pub struct ClientSettings {
    pub sleep_schedule: Box<dyn SleepSchedule>,
}

impl Default for ClientSettings {
    fn default() -> Self {
        ClientSettings {
            sleep_schedule: Box::new(CappedExponential::new(
                Duration::from_secs(1),
                2.,
                Duration::from_secs(300),
            )),
        }
    }
}
