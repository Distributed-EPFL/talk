use crate::time::SleepAgent;

use std::time::Duration;

pub trait SleepSchedule: Send + Sync {
    fn base(&self) -> Duration;
    fn next(&self, current: Duration) -> Duration;
}

impl dyn SleepSchedule {
    pub fn agent(&self) -> SleepAgent {
        SleepAgent::new(self)
    }
}
