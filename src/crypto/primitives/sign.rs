use crate::crypto::primitives::errors::{
    sign::{MalformedPublicKey, SerializeFailed, VerifyFailed},
    SignError,
};

use ed25519_dalek::{
    Keypair as EdKeyPair, PublicKey as EdPublicKey, Signature as EdSignature,
    Signer as EdSigner, Verifier as EdVerifier,
};

use rand::rngs::OsRng;

use serde::{Deserialize, Serialize};

use snafu::ResultExt;

use std::fmt::{Debug, Error as FmtError, Formatter};
use std::hash::{Hash, Hasher};

pub use ed25519_dalek::{KEYPAIR_LENGTH, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};

pub struct KeyPair(EdKeyPair);

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKey(EdPublicKey);

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(EdSignature);

impl KeyPair {
    pub fn random() -> Self {
        let keypair = EdKeyPair::generate(&mut OsRng);
        KeyPair(keypair)
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.public)
    }

    pub fn sign_raw<T>(&self, message: &T) -> Result<Signature, SignError>
    where
        T: Serialize,
    {
        let message = bincode::serialize(message).context(SerializeFailed)?;
        let signature = self.0.sign(&message);
        Ok(Signature(signature))
    }
}

impl PublicKey {
    pub fn from_bytes(
        bytes: [u8; PUBLIC_KEY_LENGTH],
    ) -> Result<Self, SignError> {
        let public_key =
            EdPublicKey::from_bytes(&bytes).context(MalformedPublicKey)?;

        Ok(PublicKey(public_key))
    }

    pub fn to_bytes(&self) -> [u8; PUBLIC_KEY_LENGTH] {
        self.0.to_bytes()
    }

    pub fn verify_raw<M>(
        &self,
        message: &M,
        signature: &Signature,
    ) -> Result<(), SignError>
    where
        M: Serialize,
    {
        let message = bincode::serialize(message).context(SerializeFailed)?;
        self.0.verify(&message, &signature.0).context(VerifyFailed)
    }

    pub fn batch_verify_raw<'m, PI, MI, M, SI>(
        public_keys: PI,
        messages: MI,
        signatures: SI,
    ) -> Result<(), SignError>
    where
        PI: IntoIterator<Item = PublicKey>,
        MI: IntoIterator<Item = &'m M>,
        M: 'm + Serialize,
        SI: IntoIterator<Item = Signature>,
    {
        let public_keys = public_keys
            .into_iter()
            .map(|public_key| public_key.0)
            .collect::<Vec<_>>();

        let messages = messages
            .into_iter()
            .map(|message| bincode::serialize(&message))
            .collect::<Result<Vec<_>, _>>()
            .context(SerializeFailed)?;

        let messages = messages
            .iter()
            .map(|message| &message[..])
            .collect::<Vec<_>>();

        let signatures = signatures
            .into_iter()
            .map(|signature| signature.0)
            .collect::<Vec<_>>();

        ed25519_dalek::verify_batch(
            &messages[..],
            &signatures[..],
            &public_keys[..],
        )
        .context(VerifyFailed)
    }
}

impl Signature {
    pub fn from_bytes(bytes: [u8; SIGNATURE_LENGTH]) -> Self {
        Signature(bytes.into())
    }

    pub fn to_bytes(&self) -> [u8; SIGNATURE_LENGTH] {
        self.0.to_bytes()
    }
}

impl Debug for PublicKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        let bytes = self
            .to_bytes()
            .iter()
            .map(|byte| format!("{:02x?}", byte))
            .collect::<Vec<_>>()
            .join("");

        if f.alternate() {
            write!(
                f,
                "PublicKey({} ... {})",
                &bytes[..8],
                &bytes[bytes.len() - 8..]
            )
        } else {
            write!(f, "PublicKey({})", bytes)
        }
    }
}

impl Debug for Signature {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        let bytes = self
            .to_bytes()
            .iter()
            .map(|byte| format!("{:02x?}", byte))
            .collect::<Vec<_>>()
            .join("");

        if f.alternate() {
            write!(
                f,
                "Signature({} ... {})",
                &bytes[..8],
                &bytes[bytes.len() - 8..]
            )
        } else {
            write!(f, "Signature({})", bytes)
        }
    }
}

impl Hash for PublicKey {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.0.to_bytes().hash(state)
    }
}

impl Hash for Signature {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.0.to_bytes().hash(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct() {
        let keypair = KeyPair::random();
        let message: u32 = 1234;
        let signature = keypair.sign_raw(&message).unwrap();

        keypair.public().verify_raw(&message, &signature).unwrap();
    }

    #[test]
    fn compromise_message() {
        let keypair = KeyPair::random();
        let message: u32 = 1234;
        let signature = keypair.sign_raw(&message).unwrap();

        let message: u32 = 1235;

        assert!(keypair.public().verify_raw(&message, &signature).is_err());
    }

    #[test]
    fn compromise_signature() {
        let keypair = KeyPair::random();
        let message: u32 = 1234;
        let signature = keypair.sign_raw(&message).unwrap();

        let mut signature = bincode::serialize(&signature).unwrap();
        signature[4] = signature[4].wrapping_add(1);
        let signature = bincode::deserialize(&signature).unwrap();

        assert!(keypair.public().verify_raw(&message, &signature).is_err());
    }
}
