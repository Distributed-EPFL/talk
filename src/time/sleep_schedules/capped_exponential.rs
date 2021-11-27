use crate::time::SleepSchedule;

use std::{cmp, time::Duration};

#[derive(Debug)]
pub struct CappedExponential {
    base: Duration,
    growth: f64,
    cap: Duration,
}

impl CappedExponential {
    pub fn new(base: Duration, growth: f64, cap: Duration) -> Self {
        CappedExponential { base, growth, cap }
    }
}

impl SleepSchedule for CappedExponential {
    fn base(&self) -> Duration {
        self.base
    }

    fn next(&self, current: Duration) -> Duration {
        cmp::min(
            Duration::from_secs_f64(current.as_secs_f64() * self.growth), // Currently, `Duration` cannot be directly multiplied by `f64`
            self.cap,
        )
    }
}
