use crate::crypto::primitives::errors::{
    sign::{SerializeFailed, VerifyFailed},
    SignError,
};

use ed25519_dalek::{
    Keypair as EdKeyPair, PublicKey as EdPublicKey, Signature as EdSignature,
    Signer as EdSigner, Verifier as EdVerifier,
};

use rand::rngs::OsRng;

use serde::{Deserialize, Serialize};

use snafu::ResultExt;

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
    pub fn verify_raw<T>(
        &self,
        message: &T,
        signature: &Signature,
    ) -> Result<(), SignError>
    where
        T: Serialize,
    {
        let message = bincode::serialize(message).context(SerializeFailed)?;
        self.0.verify(&message, &signature.0).context(VerifyFailed)
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
