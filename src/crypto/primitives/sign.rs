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

/// An ed25519 keypair.
///
/// This keypair can be used directly to sign messages via
/// [`KeyPair::sign_raw`]. The emmitted [`Signature`]s can be
/// verified against its [`PublicKey`] (see the respective
/// documentation for details)
impl KeyPair {
    /// Generates a random `KeyPair` to be used for signing.
    pub fn random() -> Self {
        let keypair = EdKeyPair::generate(&mut OsRng);
        KeyPair(keypair)
    }

    /// Returns this `KeyPair`'s `PublicKey`.
    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.public)
    }

    /// Signs a message using this `KeyPair`.
    ///
    /// # Errors
    ///
    /// If the serialization of the message fails, a `SerializeFailed`
    /// error variant will be returned.
    ///
    /// # Examples
    /// ```
    /// use talk::crypto::primitives::sign::{Signature, KeyPair, PublicKey};
    ///
    /// let alice = KeyPair::random();
    ///
    /// let message: u32 = 1234;
    ///
    /// let alice_signature = alice.sign_raw(&message).unwrap();
    ///
    /// assert!(alice_signature.verify_raw(
    ///     alice.public(),
    ///     &message,
    /// ).is_ok());
    /// ```
    pub fn sign_raw<T>(&self, message: &T) -> Result<Signature, SignError>
    where
        T: Serialize,
    {
        let message = bincode::serialize(message).context(SerializeFailed)?;
        let signature = self.0.sign(&message);
        Ok(Signature(signature))
    }
}

/// An ed25519 public key.
///
/// Used to validate a signature on a message. See the documentation
/// for [`Signature`] for details.
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
}

/// An ed25519 signature of a message.
impl Signature {
    pub fn from_bytes(bytes: [u8; SIGNATURE_LENGTH]) -> Self {
        Signature(bytes.into())
    }

    pub fn to_bytes(&self) -> [u8; SIGNATURE_LENGTH] {
        self.0.to_bytes()
    }

    /// Verifies the `Signature` of a message against a `PublicKey`.
    ///
    /// Verification succeeds if and only signature was produced by
    /// signing the message using the PublicKey's matching PrivateKey.
    ///
    /// # Errors
    ///
    /// If the serialization of the message fails, a `SerializeFailed`
    /// error variant will be returned. If the verification fails for
    /// any reason, `VerifyFailed` will be returned.
    ///
    /// # Examples
    /// ```
    /// use talk::crypto::primitives::sign::{Signature, KeyPair, PublicKey};
    ///
    /// let alice = KeyPair::random();
    ///
    /// let message: u32 = 1234;
    ///
    /// let alice_signature = alice.sign_raw(&message).unwrap();
    ///
    /// assert!(alice_signature.verify_raw(
    ///     alice.public(),
    ///     &message,
    /// ).is_ok());
    /// ```
    pub fn verify_raw<M>(
        &self,
        public_key: PublicKey,
        message: &M,
    ) -> Result<(), SignError>
    where
        M: Serialize,
    {
        let message = bincode::serialize(message).context(SerializeFailed)?;
        public_key.0.verify(&message, &self.0).context(VerifyFailed)
    }

    /// Efficiently verifies a batch of `Signature`s on messages against the
    /// given `PublicKey`s.
    ///
    /// The result is equivalent to individually verifying
    /// (i.e., by sing `Signature::verify_raw`) each triplet of `PublicKey`,
    /// message and `Signature` obtained from the iterators in the same order.
    /// Verification succeeds if and only if it succeeds for every individual
    /// triplet.
    ///
    /// # Errors
    ///
    /// If the serialization of the message fails, a `SerializeFailed`
    /// error variant will be returned. If the verification fails for
    /// any reason, `VerifyFailed` will be returned.
    ///
    /// # Examples
    /// ```
    /// use talk::crypto::primitives::sign::{Signature, KeyPair, PublicKey};
    ///
    /// let alice = KeyPair::random();
    ///
    /// let message: u32 = 1234;
    ///
    /// let alice_signature = alice.sign_raw(&message).unwrap();
    ///
    /// assert!(alice_signature.verify_raw(
    ///     alice.public(),
    ///     &message,
    /// ).is_ok());
    /// ```
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

        signature.verify_raw(keypair.public(), &message).unwrap();
    }

    #[test]
    fn compromise_message() {
        let keypair = KeyPair::random();
        let message: u32 = 1234;
        let signature = keypair.sign_raw(&message).unwrap();

        let message: u32 = 1235;

        assert!(signature.verify_raw(keypair.public(), &message).is_err());
    }

    #[test]
    fn compromise_signature() {
        let keypair = KeyPair::random();
        let message: u32 = 1234;
        let signature = keypair.sign_raw(&message).unwrap();

        let mut signature = bincode::serialize(&signature).unwrap();
        signature[4] = signature[4].wrapping_add(1);
        let signature: Signature = bincode::deserialize(&signature).unwrap();

        assert!(signature.verify_raw(keypair.public(), &message).is_err());
    }
}
