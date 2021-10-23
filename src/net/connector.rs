use async_trait::async_trait;

use crate::{crypto::Identity, net::SecureConnection};

use doomstack::Stack;

#[async_trait]
pub trait Connector: 'static + Send + Sync {
    async fn connect(
        &self,
        remote: Identity,
    ) -> Result<SecureConnection, Stack>;
}
