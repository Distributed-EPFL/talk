use async_trait::async_trait;

use crate::{
    crypto::primitives::sign::PublicKey, errors::DynError,
    net::SecureConnection,
};

#[async_trait]
pub trait Listener {
    async fn accept(
        &mut self,
    ) -> Result<(PublicKey, SecureConnection), DynError>;
}
