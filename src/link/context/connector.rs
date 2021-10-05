use async_trait::async_trait;

use crate::{
    crypto::primitives::sign::PublicKey,
    errors::DynError,
    link::context::{
        connect_dispatcher::Database,
        errors::connector::{
            ConnectionError, ConnectionFailed, ContextRefused,
        },
        ContextId, Request, Response,
    },
    net::{Connector as NetConnector, SecureConnection},
};

use snafu::ResultExt;

use std::sync::{Arc, Mutex};

pub struct Connector {
    context: ContextId,
    connector: Arc<dyn NetConnector>,
    database: Arc<Mutex<Database>>,
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
            .context(ConnectionFailed)?;

        connection
            .send(&Request::Context(self.context.clone()))
            .await
            .context(ConnectionError)?;

        match connection.receive().await.context(ConnectionError)? {
            Response::ContextAccepted => Ok(connection),
            Response::ContextRefused => {
                ContextRefused.fail().map_err(Into::into)
            }
        }
    }
}

impl Drop for Connector {
    fn drop(&mut self) {
        self.database.lock().unwrap().contexts.remove(&self.context);
    }
}
