use crate::sync::fuse::{errors::FuseBurned, FuseError};

use std::future::Future;

use tokio::sync::broadcast::Receiver;

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

    pub async fn map<F>(&mut self, future: F) -> Result<F::Output, FuseError>
    where
        F: Future,
    {
        tokio::select! {
            biased;

            _ = self.switch_off() => {
                FuseBurned.fail()
            },
            result = future => {
                Ok(result)
            }
        }
    }

    async fn switch_off(&mut self) {
        match &mut self.state {
            State::On(receiver) => receiver.recv().await.unwrap(),
            State::Off => (),
        }

        self.state = State::Off;
    }
}
