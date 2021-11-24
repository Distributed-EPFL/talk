use std::future::Future;

use tokio::{sync::broadcast::Receiver, task::JoinHandle};

pub struct Relay {
    state: State,
}

enum State {
    On(Receiver<()>),
    Off,
}

impl Relay {
    pub(in crate::sync::fuse) fn new(receiver: Receiver<()>) -> Self {
        Relay {
            state: State::On(receiver),
        }
    }

    pub fn run<F>(mut self, future: F) -> JoinHandle<Option<F::Output>>
    where
        F: 'static + Send + Future,
        F::Output: 'static + Send,
    {
        tokio::spawn(async move { self.map(future).await })
    }

    pub async fn map<F>(&mut self, future: F) -> Option<F::Output>
    where
        F: Future,
    {
        tokio::select! {
            biased;

            _ = self.wait() => {
                None
            },
            result = future => {
                Some(result)
            }
        }
    }

    pub async fn wait(&mut self) {
        match &mut self.state {
            State::On(receiver) => receiver.recv().await.unwrap(),
            State::Off => (),
        }

        self.state = State::Off;
    }
}
