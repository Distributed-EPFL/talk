use blake3::{Hash as BlakeHash, Hasher as BlakeHasher};

use crate::crypto::primitives::errors::{HasherError, SerializeError};

use serde::Serialize;

use snafu::ResultExt;

pub struct Hash(BlakeHash);
pub struct Hasher(BlakeHasher);

pub const SIZE: usize = blake3::OUT_LEN;

impl Hasher {
    pub fn new() -> Self {
        Self(BlakeHasher::new())
    }

    pub fn update<M>(&mut self, message: &M) -> Result<(), HasherError>
    where
        M: Serialize,
    {
        let message = bincode::serialize(message).context(SerializeError)?;
        self.update_raw(&message);

        Ok(())
    }

    pub fn update_raw(&mut self, chunk: &[u8]) {
        self.0.update(chunk);
    }

    pub fn finalize(self) -> Hash {
        Hash(self.0.finalize())
    }
}

pub fn hash<M>(message: &M) -> Result<Hash, HasherError>
where
    M: Serialize,
{
    let mut hasher = Hasher::new();
    hasher.update(message)?;
    Ok(hasher.finalize())
}
