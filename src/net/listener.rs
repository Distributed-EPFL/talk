use async_trait::async_trait;

use crate::{crypto::primitives::sign::PublicKey, net::SecureConnection};

#[async_trait]
pub trait Listener {
    type Error;

    async fn accept(
        &self,
    ) -> Result<(PublicKey, SecureConnection), Self::Error>;
}
