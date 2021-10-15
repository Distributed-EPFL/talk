use crate::sync::fuse::{AnyFuse, Fuse, FuseError, Relay};

use doomstack::{here, ResultExt, Top};

use std::future::Future;

pub struct Mikado {
    fuse: AnyFuse,
    relay: Relay,
}

impl Mikado {
    pub fn new() -> Self {
        let fuse = AnyFuse::new(Fuse::new());
        let relay = fuse.relay().unwrap();

        Mikado { fuse, relay }
    }

    pub fn try_clone(&self) -> Result<Self, Top<FuseError>> {
        let fuse = self.fuse.clone();
        let relay = fuse.relay().spot(here!())?;

        Ok(Mikado { fuse, relay })
    }

    pub fn depend(&self, relay: Relay) {
        self.fuse.depend(relay);
    }

    pub async fn map<F>(
        &mut self,
        future: F,
    ) -> Result<F::Output, Top<FuseError>>
    where
        F: Future,
    {
        self.relay.map(future).await
    }

    pub async fn wait(&mut self) {
        self.relay.wait().await
    }
}
