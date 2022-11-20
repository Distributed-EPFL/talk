use crate::crypto::{primitives::hash, Statement};
use doomstack::{here, Doom, ResultExt, Top};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work(u64);

#[derive(Doom)]
pub enum WorkError {
    #[doom(description("Failed to hash message"))]
    HashFailed,
    #[doom(description("Invalid nonce"))]
    InvalidNonce,
}

impl Work {
    pub fn new<S>(difficulty: u64, message: &S) -> Result<Self, Top<WorkError>>
    where
        S: Statement,
    {
        Work::new_raw(difficulty, &(S::SCOPE, S::HEADER, message))
    }

    pub fn new_raw<T>(difficulty: u64, message: &T) -> Result<Self, Top<WorkError>>
    where
        T: Serialize,
    {
        let digest = hash::hash(message).pot(WorkError::HashFailed, here!())?;
        let target = u64::MAX >> difficulty;

        // TODO: Generalize this to the multi-threaded setting
        for nonce in 0u64.. {
            let score = u64::from_le_bytes(
                hash::hash(&(digest, nonce)).unwrap().to_bytes()[0..8]
                    .try_into()
                    .unwrap(),
            );

            if score < target {
                return Ok(Work(nonce));
            }
        }

        unreachable!()
    }

    pub fn verify<S>(&self, difficulty: u64, message: &S) -> Result<(), Top<WorkError>>
    where
        S: Statement,
    {
        self.verify_raw(difficulty, &(S::SCOPE, S::HEADER, message))
    }

    pub fn verify_raw<T>(&self, difficulty: u64, message: &T) -> Result<(), Top<WorkError>>
    where
        T: Serialize,
    {
        let digest = hash::hash(message).pot(WorkError::HashFailed, here!())?;
        let target = u64::MAX >> difficulty;

        let score = u64::from_le_bytes(
            hash::hash(&(digest, self.0)).unwrap().to_bytes()[0..8]
                .try_into()
                .unwrap(),
        );

        if score < target {
            Ok(())
        } else {
            WorkError::InvalidNonce.fail().spot(here!())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easy() {
        for message in 0u32..32u32 {
            let work = Work::new_raw(10, &message).unwrap();
            work.verify_raw(10, &message).unwrap()
        }
    }

    #[test]
    #[ignore]
    fn hard() {
        let work = Work::new_raw(24, &42u32).unwrap();
        work.verify_raw(24, &42u32).unwrap()
    }
}
