use async_trait::async_trait;

use crate::{
    crypto::primitives::sign::PublicKey, errors::DynError,
    net::SecureConnection,
};

#[async_trait]
pub trait Listener: 'static + Send + Sync {
    async fn accept(
        &mut self,
    ) -> Result<(PublicKey, SecureConnection), DynError>;
}
