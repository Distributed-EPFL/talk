use async_trait::async_trait;

use crate::{
    crypto::primitives::sign::PublicKey,
    errors::DynError,
    link::context::{listen_dispatcher::Database, ContextId},
    net::{Listener as NetListener, SecureConnection},
    sync::fuse::Fuse,
};

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::Receiver;

type Outlet = Receiver<(PublicKey, SecureConnection)>;

pub struct Listener {
    context: ContextId,
    outlet: Outlet,
    database: Arc<Mutex<Database>>,
    fuse: Arc<Fuse>,
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
            fuse,
        }
    }
}

#[async_trait]
impl NetListener for Listener {
    async fn accept(
        &mut self,
    ) -> Result<(PublicKey, SecureConnection), DynError> {
        // In order for `self.outlet.recv()` to return `None`, the corresponding
        // `inlet` would need to be dropped from `self.database.inlets`. This,
        // however, happens only when `Listener` is dropped, which cannot happen
        // while `accept()` is being executed.
        Ok(self.outlet.recv().await.unwrap())
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.database.lock().unwrap().inlets.remove(&self.context);
    }
}