use crate::sync::fuse::{Fuse, FuseError, Relay};

use doomstack::{here, Doom, ResultExt, Top};

use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AnyFuse {
    state: Arc<Mutex<State>>,
}

enum State {
    Intact(Fuse),
    Burned,
}

impl AnyFuse {
    pub fn new(fuse: Fuse) -> Self {
        AnyFuse {
            state: Arc::new(Mutex::new(State::Intact(fuse))),
        }
    }

    pub fn relay(&self) -> Result<Relay, Top<FuseError>> {
        match &*self.state.lock().unwrap() {
            State::Intact(fuse) => Ok(fuse.relay()),
            State::Burned => FuseError::FuseBurned.fail().spot(here!()),
        }
    }

    pub fn burn(self) {
        // This function is empty on purpose: `Drop::drop(self)` is called here
    }
}

impl Drop for AnyFuse {
    fn drop(&mut self) {
        *self.state.lock().unwrap() = State::Burned
    }
}
