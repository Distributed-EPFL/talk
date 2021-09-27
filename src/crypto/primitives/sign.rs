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
