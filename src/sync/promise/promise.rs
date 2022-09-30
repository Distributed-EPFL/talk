use crate::sync::promise::Solver;

use tokio::sync::oneshot::{self, Receiver as OneShotReceiver};

pub struct Promise<T> {
    state: State<T>,
}

enum State<T> {
    Solved(T),
    Dropped,
    Pending(OneShotReceiver<T>),
}

impl<T> Promise<T> {
    pub fn solved(value: T) -> Self {
        Promise {
            state: State::Solved(value),
        }
    }

    pub fn pending() -> (Promise<T>, Solver<T>) {
        let (inlet, outlet) = oneshot::channel();

        let promise = Promise {
            state: State::Pending(outlet),
        };

        let solver = Solver::from_inlet(inlet);

        (promise, solver)
    }

    pub async fn wait(&mut self) {
        let value = if let State::Pending(outlet) = &mut self.state {
            outlet.await
        } else {
            return;
        };

        self.state = if let Ok(value) = value {
            State::Solved(value)
        } else {
            State::Dropped
        };
    }

    pub async fn take(mut self) -> Option<T> {
        self.wait().await;

        match self.state {
            State::Solved(value) => Some(value),
            State::Dropped => None,
            State::Pending(_) => unreachable!(),
        }
    }

    pub async fn as_ref(&mut self) -> Option<&T> {
        self.wait().await;

        match &self.state {
            State::Solved(value) => Some(value),
            State::Dropped => None,
            State::Pending(_) => unreachable!(),
        }
    }

    pub async fn as_mut(&mut self) -> Option<&mut T> {
        self.wait().await;

        match &mut self.state {
            State::Solved(value) => Some(value),
            State::Dropped => None,
            State::Pending(_) => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn solved() {
        let promise = Promise::solved(33u32);
        assert_eq!(promise.take().await.unwrap(), 33);

        let mut promise = Promise::solved(33u32);
        assert_eq!(promise.as_ref().await.unwrap(), &33);

        let mut promise = Promise::solved(33u32);
        assert_eq!(promise.as_mut().await.unwrap(), &mut 33);
    }

    #[tokio::test]
    async fn pending() {
        let (promise, solver) = Promise::<u32>::pending();

        tokio::spawn(async move {
            solver.solve(33);
        });

        assert_eq!(promise.take().await.unwrap(), 33);

        let (mut promise, solver) = Promise::<u32>::pending();

        tokio::spawn(async move {
            solver.solve(33);
        });

        assert_eq!(promise.as_ref().await.unwrap(), &33);

        let (mut promise, solver) = Promise::<u32>::pending();

        tokio::spawn(async move {
            solver.solve(33);
        });

        assert_eq!(promise.as_mut().await.unwrap(), &mut 33);
    }
}
