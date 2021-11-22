use doomstack::{here, Doom, ResultExt, Top};

use serde::{Deserialize, Serialize};

use vdf::{InvalidProof, VDFParams, WesolowskiVDFParams, VDF};

const BITS: u16 = 2048;

/// WARNING: This primitive should not be used in production code,
/// as it can be made to panic with malformed inputs
#[derive(Serialize, Deserialize)]
pub struct Delay(Vec<u8>);

#[derive(Doom)]
pub enum DelayError {
    #[doom(description("Failed to serialize: {}", source))]
    #[doom(wrap(serialize_failed))]
    SerializeFailed { source: bincode::Error },

    #[doom(description("Invalid proof"))]
    #[doom(wrap(invalid_proof))]
    InvalidProof { source: InvalidProof },
}

impl Delay {
    pub fn new<T>(difficulty: u64, message: &T) -> Result<Self, Top<DelayError>>
    where
        T: Serialize,
    {
        let message = bincode::serialize(message)
            .map_err(DelayError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        let solution = WesolowskiVDFParams(BITS)
            .new()
            .solve(&message, difficulty)
            .unwrap();

        Ok(Delay(solution))
    }

    pub fn verify<T>(
        &self,
        difficulty: u64,
        message: &T,
    ) -> Result<(), Top<DelayError>>
    where
        T: Serialize,
    {
        let message = bincode::serialize(message)
            .map_err(DelayError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        WesolowskiVDFParams(BITS)
            .new()
            .verify(&message, difficulty, &self.0)
            .map_err(DelayError::invalid_proof)
            .map_err(Doom::into_top)
            .spot(here!())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief() {
        let delay = Delay::new(100, &42u32).unwrap();
        delay.verify(100, &42u32).unwrap();
    }

    #[test]
    #[ignore]
    fn long() {
        let delay = Delay::new(100000, &42u32).unwrap();
        delay.verify(100000, &42u32).unwrap();
    }
}
