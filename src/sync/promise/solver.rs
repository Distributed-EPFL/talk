use tokio::sync::oneshot::Sender as OneShotSender;

pub struct Solver<T> {
    inlet: OneShotSender<T>,
}

impl<T> Solver<T> {
    pub(in crate::sync::promise) fn from_inlet(inlet: OneShotSender<T>) -> Self {
        Solver { inlet }
    }

    pub fn solve(self, value: T) {
        let _ = self.inlet.send(value);
    }
}
