use crate::{
    crypto::Identity,
    link::context::{listen_dispatcher::Database, ContextId},
    net::{Listener as NetListener, SecureConnection},
    sync::fuse::Fuse,
};
use async_trait::async_trait;
use doomstack::Stack;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

type Outlet = Receiver<(Identity, SecureConnection)>;

pub struct Listener {
    context: ContextId,
    outlet: Outlet,
    database: Arc<Mutex<Database>>,
    _fuse: Arc<Fuse>,
}

impl Listener {
    pub(in crate::link::context) fn new(
        context: ContextId,
        outlet: Outlet,
        database: Arc<Mutex<Database>>,
        fuse: Arc<Fuse>,
    ) -> Self {
        Listener {
            context,
            outlet,
            database,
            _fuse: fuse,
        }
    }
}

#[async_trait]
impl NetListener for Listener {
    async fn accept(&mut self) -> Result<(Identity, SecureConnection), Stack> {
        // In order for `self.outlet.recv()` to return `None`, the corresponding
        // `inlet` would need to be dropped from `self.database.inlets`. This,
        // however, happens only when `Listener` is dropped, which cannot happen
        // while `accept()` is being executed.
        Ok(self.outlet.recv().await.unwrap())
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.database.lock().inlets.remove(&self.context);
    }
}
