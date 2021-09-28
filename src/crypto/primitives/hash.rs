use blake3::{Hash as BlakeHash, Hasher as BlakeHasher};

use crate::crypto::primitives::errors::{hash::SerializeFailed, HashError};

use serde::{Deserialize, Serialize};

use snafu::ResultExt;

use std::fmt::{Debug, Error as FmtError, Formatter};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash(#[serde(with = "SerdeBlakeHash")] BlakeHash);

pub struct Hasher(BlakeHasher);

pub const HASH_LENGTH: usize = blake3::OUT_LEN;

impl Hash {
    pub fn from_bytes(bytes: [u8; HASH_LENGTH]) -> Self {
        Hash(BlakeHash::from(bytes))
    }

    pub fn to_bytes(&self) -> [u8; HASH_LENGTH] {
        *self.0.as_bytes()
    }
}

impl Hasher {
    pub fn new() -> Self {
        Self(BlakeHasher::new())
    }

    pub fn update<M>(&mut self, message: &M) -> Result<(), HashError>
    where
        M: Serialize,
    {
        let message = bincode::serialize(message).context(SerializeFailed)?;
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

pub fn hash<M>(message: &M) -> Result<Hash, HashError>
where
    M: Serialize,
{
    let mut hasher = Hasher::new();
    hasher.update(message)?;
    Ok(hasher.finalize())
}

impl Debug for Hash {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        let bytes = self
            .to_bytes()
            .iter()
            .map(|byte| format!("{:02x?}", byte))
            .collect::<Vec<_>>()
            .join("");

        write!(f, "Hash({})", bytes)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "BlakeHash")]
struct SerdeBlakeHash(
    #[serde(getter = "BlakeHash::as_bytes")] [u8; HASH_LENGTH],
);

impl Into<BlakeHash> for SerdeBlakeHash {
    fn into(self) -> BlakeHash {
        BlakeHash::from(self.0)
    }
}
