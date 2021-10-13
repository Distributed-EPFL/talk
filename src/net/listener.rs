use async_trait::async_trait;

use crate::{crypto::primitives::sign::PublicKey, net::SecureConnection};

use doomstack::Stack;

#[async_trait]
pub trait Listener: 'static + Send + Sync {
    async fn accept(&mut self) -> Result<(PublicKey, SecureConnection), Stack>;
}
