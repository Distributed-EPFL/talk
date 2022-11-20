use crate::time::SleepSchedule;
use std::time::Duration;

#[derive(Debug)]
pub struct Constant {
    interval: Duration,
}

impl Constant {
    pub fn new(interval: Duration) -> Self {
        Constant { interval }
    }
}

impl SleepSchedule for Constant {
    fn base(&self) -> Duration {
        self.interval
    }

    fn next(&self, _current: Duration) -> Duration {
        self.interval
    }
}
