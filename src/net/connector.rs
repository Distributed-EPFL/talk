use async_trait::async_trait;

use crate::{crypto::primitives::sign::PublicKey, net::SecureConnection};

#[async_trait]
pub trait Connector {
    type Error;

    async fn connect(
        &self,
        remote: PublicKey,
    ) -> Result<SecureConnection, Self::Error>;
}
