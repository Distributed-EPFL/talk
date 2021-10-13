use async_trait::async_trait;

use crate::{
    crypto::primitives::sign::PublicKey,
    errors::DynError,
    link::context::{
        connect_dispatcher::Database, ContextId, Request, Response,
    },
    net::{Connector as NetConnector, SecureConnection},
};

use doomstack::{here, Doom, ResultExt};

use std::sync::{Arc, Mutex};

pub struct Connector {
    context: ContextId,
    connector: Arc<dyn NetConnector>,
    database: Arc<Mutex<Database>>,
}

#[derive(Doom)]
pub enum ConnectorError {
    #[doom(description("Connection error"))]
    ConnectionError,
    #[doom(description("Failed to connect: {}", source))]
    #[doom(wrap(connect_failed))]
    ConnectFailed { source: DynError },
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
    async fn connect(
        &self,
        remote: PublicKey,
    ) -> Result<SecureConnection, DynError> {
        let mut connection = self
            .connector
            .connect(remote)
            .await
            .map_err(ConnectorError::connect_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

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
        self.database.lock().unwrap().contexts.remove(&self.context);
    }
}
