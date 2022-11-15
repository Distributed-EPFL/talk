use doomstack::{here, Doom, ResultExt, Top};

use ed25519_dalek::{
    Keypair as EdKeyPair, PublicKey as EdPublicKey, Signature as EdSignature, Signer as EdSigner,
    Verifier as EdVerifier,
};

use rand::{rngs::OsRng, CryptoRng, RngCore};

use serde::{Deserialize, Serialize};

use std::{
    cmp::{Ord, Ordering, PartialOrd},
    convert::TryInto,
    fmt,
    fmt::{Debug, Formatter},
    hash::{Hash, Hasher},
};

pub use ed25519_dalek::{KEYPAIR_LENGTH, PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH, SIGNATURE_LENGTH};

#[derive(Serialize, Deserialize)]
pub struct KeyPair(EdKeyPair);

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKey(EdPublicKey);

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(EdSignature);

pub trait Signer {
    fn public_key(&self) -> &PublicKey;
}

#[derive(Doom)]
pub enum SignError {
    #[doom(description("Incorrect buffer size"))]
    IncorrectBufferSize,
    #[doom(description("Malformed public key: {}", source))]
    #[doom(wrap(malformed_public_key))]
    MalformedPublicKey {
        source: ed25519_dalek::SignatureError,
    },
    #[doom(description("Malformed keypair: {}", source))]
    #[doom(wrap(malformed_keypair))]
    MalformedKeyPair {
        source: ed25519_dalek::SignatureError,
    },
    #[doom(description("Failed to serialize: {}", source))]
    #[doom(wrap(serialize_failed))]
    SerializeFailed { source: bincode::Error },
    #[doom(description("Failed to `verify` signature: {}", source))]
    #[doom(wrap(verify_failed))]
    VerifyFailed {
        source: ed25519_dalek::SignatureError,
    },
}

/// An ed25519 keypair.
///
/// This keypair can be used directly to sign messages via
/// [`KeyPair::sign_raw`]. The emmitted [`Signature`]s can be
/// verified against its [`PublicKey`] (see the respective
/// documentation for details)
impl KeyPair {
    /// Generates a random `KeyPair` to be used for signing.
    pub fn random() -> Self {
        KeyPair::from_rng(&mut OsRng)
    }

    pub fn from_rng<R>(rng: &mut R) -> Self
    where
        R: CryptoRng + RngCore,
    {
        let keypair = EdKeyPair::generate(rng);
        KeyPair(keypair)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Top<SignError>> {
        if bytes.len() != KEYPAIR_LENGTH {
            return SignError::IncorrectBufferSize.fail().spot(here!());
        }

        let keypair = EdKeyPair::from_bytes(bytes)
            .map_err(SignError::malformed_keypair)
            .map_err(Doom::into_top)
            .spot(here!())?;

        Ok(KeyPair(keypair))
    }

    /// Returns this `KeyPair`'s `PublicKey`.
    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.public)
    }

    pub fn to_bytes(&self) -> [u8; KEYPAIR_LENGTH] {
        self.0.to_bytes()
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
    ///     &alice.public(),
    ///     &message,
    /// ).is_ok());
    /// ```
    pub fn sign_raw<T>(&self, message: &T) -> Result<Signature, Top<SignError>>
    where
        T: Serialize,
    {
        let message = bincode::serialize(message)
            .map_err(SignError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        let signature = self.0.sign(&message);
        Ok(Signature(signature))
    }
}

/// An ed25519 public key.
///
/// Used to validate a signature on a message. See the documentation
/// for [`Signature`] for details.
impl PublicKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Top<SignError>> {
        if bytes.len() != PUBLIC_KEY_LENGTH {
            return SignError::IncorrectBufferSize.fail().spot(here!());
        }

        let public_key = EdPublicKey::from_bytes(bytes)
            .map_err(SignError::malformed_public_key)
            .map_err(Doom::into_top)
            .spot(here!())?;

        Ok(PublicKey(public_key))
    }

    pub fn to_bytes(&self) -> [u8; PUBLIC_KEY_LENGTH] {
        self.0.to_bytes()
    }
}

/// An ed25519 signature of a message.
impl Signature {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Top<SignError>> {
        Ok(Signature(
            bytes
                .try_into()
                .map_err(|_| SignError::IncorrectBufferSize.into_top())
                .spot(here!())?,
        ))
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
    ///     &alice.public(),
    ///     &message,
    /// ).is_ok());
    /// ```
    pub fn verify_raw<M>(&self, public_key: &PublicKey, message: &M) -> Result<(), Top<SignError>>
    where
        M: Serialize,
    {
        let message = bincode::serialize(message)
            .map_err(SignError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        public_key
            .0
            .verify(&message, &self.0)
            .map_err(SignError::verify_failed)
            .map_err(Doom::into_top)
            .spot(here!())
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
    ///     &alice.public(),
    ///     &message,
    /// ).is_ok());
    /// ```
    pub fn batch_verify_raw<'m, PI, MI, M, SI>(
        public_keys: PI,
        messages: MI,
        signatures: SI,
    ) -> Result<(), Top<SignError>>
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
            .map_err(SignError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        let messages = messages
            .iter()
            .map(|message| &message[..])
            .collect::<Vec<_>>();

        let signatures = signatures
            .into_iter()
            .map(|signature| signature.0)
            .collect::<Vec<_>>();

        ed25519_dalek::verify_batch(&messages[..], &signatures[..], &public_keys[..])
            .map_err(SignError::verify_failed)
            .map_err(Doom::into_top)
            .spot(here!())
    }
}

impl Signer for PublicKey {
    fn public_key(&self) -> &PublicKey {
        self
    }
}

impl Debug for PublicKey {
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

impl PartialOrd for PublicKey {
    fn partial_cmp(&self, rho: &PublicKey) -> Option<Ordering> {
        Some(self.cmp(&rho))
    }
}

impl Ord for PublicKey {
    fn cmp(&self, rho: &PublicKey) -> Ordering {
        self.to_bytes().cmp(&rho.to_bytes())
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

        signature.verify_raw(&keypair.public(), &message).unwrap();
    }

    #[test]
    fn compromise_message() {
        let keypair = KeyPair::random();
        let message: u32 = 1234;
        let signature = keypair.sign_raw(&message).unwrap();

        let message: u32 = 1235;

        assert!(signature.verify_raw(&keypair.public(), &message).is_err());
    }

    #[test]
    fn compromise_signature() {
        let keypair = KeyPair::random();
        let message: u32 = 1234;
        let signature = keypair.sign_raw(&message).unwrap();

        let mut signature = bincode::serialize(&signature).unwrap();
        signature[4] = signature[4].wrapping_add(1);
        let signature: Signature = bincode::deserialize(&signature).unwrap();

        assert!(signature.verify_raw(&keypair.public(), &message).is_err());
    }

    #[test]
    fn serialize_keypair() {
        let original = KeyPair::random();
        let serialized = bincode::serialize(&original).unwrap();
        let deserialized = bincode::deserialize::<KeyPair>(serialized.as_slice()).unwrap();

        assert_eq!(original.0.to_bytes(), deserialized.0.to_bytes());

        let message = 42u64;
        let signature = deserialized.sign_raw(&message).unwrap();
        signature.verify_raw(&original.public(), &message).unwrap();
    }
}
