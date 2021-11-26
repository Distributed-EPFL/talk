use async_trait::async_trait;

use crate::{
    crypto::Identity,
    link::context::{connect_dispatcher::Database, ContextId, Request, Response},
    net::{Connector as NetConnector, SecureConnection},
};

use doomstack::{here, Doom, ResultExt, Stack};

use parking_lot::Mutex;

use std::sync::Arc;

pub struct Connector {
    context: ContextId,
    connector: Arc<dyn NetConnector>,
    database: Arc<Mutex<Database>>,
}

#[derive(Doom)]
pub enum ConnectorError {
    #[doom(description("Connection error"))]
    ConnectionError,
    #[doom(description("Failed to connect"))]
    ConnectFailed,
    #[doom(description("Context refused"))]
    ContextRefused,
}

impl Connector {
    pub(in crate::link::context) fn new(
        context: ContextId,
        connector: Arc<dyn NetConnector>,
        database: Arc<Mutex<Database>>,
    ) -> Self {
        Connector {
            context,
            connector,
            database,
        }
    }
}

#[async_trait]
impl NetConnector for Connector {
    async fn connect(&self, remote: Identity) -> Result<SecureConnection, Stack> {
        let mut connection = self
            .connector
            .connect(remote)
            .await
            .pot(ConnectorError::ConnectFailed, here!())?;

        connection
            .send(&Request::Context(self.context.clone()))
            .await
            .pot(ConnectorError::ConnectionError, here!())?;

        match connection
            .receive()
            .await
            .pot(ConnectorError::ConnectionError, here!())?
        {
            Response::ContextAccepted => Ok(connection),
            Response::ContextRefused => ConnectorError::ContextRefused
                .fail()
                .spot(here!())
                .map_err(Into::into),
        }
    }
}

impl Drop for Connector {
    fn drop(&mut self) {
        self.database.lock().contexts.remove(&self.context);
    }
}
