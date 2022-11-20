use crate::{crypto::Identity, net::SecureConnection};
use async_trait::async_trait;
use doomstack::Stack;

#[async_trait]
pub trait Connector: 'static + Send + Sync {
    async fn connect(&self, remote: Identity) -> Result<SecureConnection, Stack>;
}
