use async_trait::async_trait;

use crate::{
    crypto::primitives::sign::PublicKey, errors::DynError,
    net::SecureConnection,
};

#[async_trait]
pub trait Connector: 'static + Send + Sync {
    async fn connect(
        &self,
        remote: PublicKey,
    ) -> Result<SecureConnection, DynError>;
}
