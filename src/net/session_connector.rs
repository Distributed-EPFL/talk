use crate::{
    crypto::Identity,
    net::{Connector as NetConnector, SecureConnection, Session},
    sync::fuse::Fuse,
};

use doomstack::Stack;

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use tokio::sync::mpsc::{self, Receiver, Sender};

type ReturnInlet = Sender<SecureConnection>;
type ReturnOutlet = Receiver<SecureConnection>;

pub struct SessionConnector {
    connector: Arc<dyn NetConnector>,
    return_inlet: ReturnInlet,
    _fuse: Fuse,
}

impl SessionConnector {
    pub fn new<C>(connector: C) -> Self
    where
        C: NetConnector,
    {
        let connector = Arc::new(connector);

        let (return_inlet, _return_outlet) = mpsc::channel(32); // TODO: Add settings

        let fuse = Fuse::new();

        SessionConnector {
            connector,
            return_inlet,
            _fuse: fuse,
        }
    }

    async fn connect(&self, remote: Identity) -> Result<Session, Stack> {
        let connection = self.connector.connect(remote).await?;
        Ok(Session::new(connection, self.return_inlet.clone()))
    }
}
