use crate::time::SleepSchedule;

use std::time::Duration;

use tokio::time;

pub struct SleepAgent<'a> {
    schedule: &'a dyn SleepSchedule,
    current: Duration,
}

impl<'a> SleepAgent<'a> {
    pub(in crate::time) fn new(schedule: &'a dyn SleepSchedule) -> Self {
        SleepAgent {
            schedule,
            current: schedule.base(),
        }
    }

    pub async fn step(&mut self) {
        time::sleep(self.current).await;
        self.current = self.schedule.next(self.current);
    }

    pub fn reset(&mut self) {
        self.current = self.schedule.base();
    }
}
