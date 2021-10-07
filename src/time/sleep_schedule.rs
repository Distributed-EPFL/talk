use crate::time::SleepAgent;

use std::fmt::Debug;
use std::time::Duration;

pub trait SleepSchedule: Debug + Send + Sync {
    fn base(&self) -> Duration;
    fn next(&self, current: Duration) -> Duration;
}

impl dyn SleepSchedule {
    pub fn agent(&self) -> SleepAgent {
        SleepAgent::new(self)
    }
}
