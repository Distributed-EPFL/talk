use crate::crypto::primitives::hash::{Hash, HASH_LENGTH};

use serde::{Deserialize, Serialize};

use std::{
    fmt,
    fmt::{Debug, Formatter},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Identity(Hash);

impl Identity {
    pub(in crate::crypto) fn from_hash(hash: Hash) -> Self {
        Identity(hash)
    }

    pub fn from_bytes(bytes: [u8; HASH_LENGTH]) -> Self {
        Identity(Hash::from_bytes(bytes))
    }

    pub fn to_bytes(&self) -> [u8; HASH_LENGTH] {
        self.0.to_bytes()
    }
}

impl Debug for Identity {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        let bytes = self
            .to_bytes()
            .iter()
            .map(|byte| format!("{:02x?}", byte))
            .collect::<Vec<_>>()
            .join("");

        if f.alternate() {
            write!(
                f,
                "Identity({} ... {})",
                &bytes[..8],
                &bytes[bytes.len() - 8..]
            )
        } else {
            write!(f, "Identity({})", bytes)
        }
    }
}
