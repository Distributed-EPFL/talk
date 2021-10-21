use crate::crypto::primitives::hash::{hash, Hash, HashError, HASH_LENGTH};

use doomstack::{here, ResultExt, Top};

use serde::{Deserialize, Serialize};

use std::fmt;
use std::fmt::{Debug, Formatter};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SetHash(Hash);

impl SetHash {
    pub fn empty() -> Self {
        SetHash(Hash::from_bytes([0; HASH_LENGTH]))
    }

    pub fn add<E>(self, element: &E) -> Result<Self, Top<HashError>>
    where
        E: Serialize,
    {
        Ok(SetHash(SetHash::combine(
            self.0,
            hash(element).spot(here!())?,
        )))
    }

    pub fn remove<E>(self, element: &E) -> Result<Self, Top<HashError>>
    where
        E: Serialize,
    {
        Ok(SetHash(SetHash::combine(
            self.0,
            hash(element).spot(here!())?,
        )))
    }

    pub fn union(self, set: SetHash) -> Self {
        SetHash(SetHash::combine(self.0, set.0))
    }

    pub fn difference(self, set: SetHash) -> Self {
        SetHash(SetHash::combine(self.0, set.0))
    }

    pub fn from_bytes(bytes: [u8; HASH_LENGTH]) -> Self {
        SetHash(Hash::from_bytes(bytes))
    }

    pub fn to_bytes(&self) -> [u8; HASH_LENGTH] {
        self.0.to_bytes()
    }

    fn combine(lho: Hash, rho: Hash) -> Hash {
        let lho = lho.to_bytes();
        let rho = rho.to_bytes();

        let mut ret = [0; HASH_LENGTH];
        for i in 0..HASH_LENGTH {
            ret[i] = lho[i] ^ rho[i];
        }

        Hash::from_bytes(ret)
    }
}

impl Debug for SetHash {
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
                "HashSet({} ... {})",
                &bytes[..8],
                &bytes[bytes.len() - 8..]
            )
        } else {
            write!(f, "HashSet({})", bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        assert_eq!(SetHash::empty(), SetHash::empty());
    }

    #[test]
    fn add_commute() {
        assert_eq!(
            SetHash::empty().add(&3u32).unwrap().add(&4u32).unwrap(),
            SetHash::empty().add(&4u32).unwrap().add(&3u32).unwrap()
        );

        assert_eq!(
            SetHash::empty()
                .add(&3u32)
                .unwrap()
                .add(&4u32)
                .unwrap()
                .add(&5u32)
                .unwrap(),
            SetHash::empty()
                .add(&4u32)
                .unwrap()
                .add(&5u32)
                .unwrap()
                .add(&3u32)
                .unwrap()
        );
    }

    #[test]
    fn add_remove() {
        assert_eq!(
            SetHash::empty().add(&5u32).unwrap().remove(&5u32).unwrap(),
            SetHash::empty()
        );

        assert_eq!(
            SetHash::empty()
                .add(&3u32)
                .unwrap()
                .add(&5u32)
                .unwrap()
                .remove(&5u32)
                .unwrap(),
            SetHash::empty().add(&3u32).unwrap()
        );

        assert_eq!(
            SetHash::empty()
                .add(&5u32)
                .unwrap()
                .add(&3u32)
                .unwrap()
                .remove(&5u32)
                .unwrap(),
            SetHash::empty().add(&3u32).unwrap()
        );
    }

    #[test]
    fn union_commute() {
        let even = SetHash::empty().add(&0i32).unwrap().add(&2i32).unwrap();
        let odd = SetHash::empty().add(&1i32).unwrap().add(&3i32).unwrap();

        let negative =
            SetHash::empty().add(&-11i32).unwrap().add(&-2i32).unwrap();

        assert_eq!(even.union(odd), odd.union(even));

        assert_eq!(
            even.union(odd).union(negative),
            odd.union(negative).union(even)
        );
    }

    #[test]
    fn union_difference() {
        let even = SetHash::empty().add(&0i32).unwrap().add(&2i32).unwrap();
        let odd = SetHash::empty().add(&1i32).unwrap().add(&3i32).unwrap();

        let negative =
            SetHash::empty().add(&-11i32).unwrap().add(&-2i32).unwrap();

        assert_eq!(even.difference(even), SetHash::empty());
        assert_eq!(even.union(odd).difference(odd), even);

        assert_eq!(
            even.union(negative).union(odd).difference(negative),
            even.union(odd)
        );
    }
}
