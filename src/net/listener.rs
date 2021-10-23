use async_trait::async_trait;

use crate::{crypto::Identity, net::SecureConnection};

use doomstack::Stack;

#[async_trait]
pub trait Listener: 'static + Send + Sync {
    async fn accept(&mut self) -> Result<(Identity, SecureConnection), Stack>;
}
