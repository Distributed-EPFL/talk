use blake3::{Hash as BlakeHash, Hasher as BlakeHasher};
use doomstack::{here, Doom, ResultExt, Top};
use serde::{Deserialize, Serialize};
use std::{
    cmp::{Ord, Ordering, PartialOrd},
    fmt,
    fmt::{Debug, Formatter},
};

pub const HASH_LENGTH: usize = blake3::OUT_LEN;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash(#[serde(with = "SerdeBlakeHash")] BlakeHash);

pub struct Hasher(BlakeHasher);

#[derive(Doom)]
pub enum HashError {
    #[doom(description("Failed to serialize: {:?}", source))]
    #[doom(wrap(serialize_failed))]
    SerializeFailed { source: bincode::Error },
}

impl Hash {
    pub fn from_bytes(bytes: [u8; HASH_LENGTH]) -> Self {
        Hash(BlakeHash::from(bytes))
    }

    pub fn to_bytes(&self) -> [u8; HASH_LENGTH] {
        *self.0.as_bytes()
    }
}

/// An incremental hash state that can accept any number of writes.
///
/// # Examples
///
/// ```
/// // Hash an input incrementally.
///
/// use talk::crypto::primitives::hash::{self, Hasher};
///
/// let mut hasher: Hasher = Hasher::new();
/// hasher.update_raw(b"foo");
/// hasher.update_raw(b"bar");
/// assert_eq!(hasher.finalize(), hash::hash(b"foobar").unwrap());
///
/// let mut hasher: Hasher = Hasher::new();
/// hasher.update(&String::from("foobar"));
/// assert_eq!(hasher.finalize(), hash::hash(&String::from("foobar")).unwrap());
/// ```
impl Hasher {
    /// Construct a new `Hasher` for the regular hash function
    pub fn new() -> Self {
        Self(BlakeHasher::new())
    }

    /// Add input bytes to the hash state. You can call this any number of times.
    ///
    /// # Errors
    ///
    /// If the serialization of the data fails, a `SerializeFailed`
    /// error variant will be returned.
    ///
    /// # Examples
    /// ```
    /// use talk::crypto::primitives::hash::Hasher;
    ///
    /// let some_data = 42u32;
    ///
    /// let mut hasher: Hasher = Hasher::new();
    ///
    /// hasher.update(&some_data);
    ///
    /// hasher.finalize();
    /// ```
    pub fn update<D>(&mut self, data: &D) -> Result<(), Top<HashError>>
    where
        D: Serialize,
    {
        let data = bincode::serialize(data)
            .map_err(HashError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        self.update_raw(&data);

        Ok(())
    }

    /// Add input bytes to the hash state. You can call this any number of times.
    pub fn update_raw(&mut self, chunk: &[u8]) {
        self.0.update(chunk);
    }

    /// Finalize the hash state and return the `Hash` of the input.
    /// This method is idempotent. Calling it twice will give the same result.
    /// You can also add more input and finalize again.
    pub fn finalize(self) -> Hash {
        Hash(self.0.finalize())
    }
}

/// The default hash function.
pub fn hash<M>(message: &M) -> Result<Hash, Top<HashError>>
where
    M: Serialize,
{
    let mut hasher = Hasher::new();
    hasher.update(message)?;
    Ok(hasher.finalize())
}

impl Debug for Hash {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        let bytes = self
            .to_bytes()
            .iter()
            .map(|byte| format!("{:02x?}", byte))
            .collect::<Vec<_>>()
            .join("");

        if f.alternate() {
            write!(f, "Hash({} ... {})", &bytes[..8], &bytes[bytes.len() - 8..])
        } else {
            write!(f, "Hash({})", bytes)
        }
    }
}

impl PartialOrd for Hash {
    fn partial_cmp(&self, rho: &Hash) -> Option<Ordering> {
        Some(self.cmp(&rho))
    }
}

impl Ord for Hash {
    fn cmp(&self, rho: &Hash) -> Ordering {
        self.to_bytes().cmp(&rho.to_bytes())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "BlakeHash")]
struct SerdeBlakeHash(#[serde(getter = "BlakeHash::as_bytes")] [u8; HASH_LENGTH]);

impl Into<BlakeHash> for SerdeBlakeHash {
    fn into(self) -> BlakeHash {
        BlakeHash::from(self.0)
    }
}
