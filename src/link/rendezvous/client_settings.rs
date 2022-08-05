use crate::{
    net::traits::ConnectSettings,
    time::{sleep_schedules::CappedExponential, SleepSchedule},
};

use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone)]
pub struct ClientSettings {
    pub sleep_schedule: Arc<dyn SleepSchedule>,
    pub connect: ConnectSettings,
}

impl Default for ClientSettings {
    fn default() -> Self {
        ClientSettings {
            sleep_schedule: Arc::new(CappedExponential::new(
                Duration::from_secs(1),
                2.,
                Duration::from_secs(300),
            )),
            connect: ConnectSettings::default(),
        }
    }
}
