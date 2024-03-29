use crate::crypto::primitives::adapters::{BlstError, BlstErrorAdapter};
use blst::min_pk::{
    AggregateSignature as BlstAggregateSignature, PublicKey as BlstPublicKey,
    SecretKey as BlstSecretKey, Signature as BlstSignature,
};
use doomstack::{here, Doom, ResultExt, Top};
use rand::{rngs::OsRng, CryptoRng, RngCore};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    cmp::{Ord, Ordering, PartialOrd},
    fmt,
    fmt::{Debug, Formatter},
    hash::{Hash, Hasher},
};

pub const PUBLIC_KEY_LENGTH: usize = 96;
pub const SECRET_KEY_LENGTH: usize = 32;
pub const SIGNATURE_LENGTH: usize = 192;

pub const KEYPAIR_LENGTH: usize = PUBLIC_KEY_LENGTH + SECRET_KEY_LENGTH;

const BLST_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";

pub struct KeyPair {
    public: BlstPublicKey,
    secret: BlstSecretKey,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PublicKey(BlstPublicKey);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature(BlstSignature);

pub trait Signer {
    fn public_key(&self) -> &PublicKey;
}

#[derive(Doom)]
pub enum MultiError {
    #[doom(description("Failed to `aggregate` signatures: {}", source))]
    #[doom(wrap(aggregate_failed))]
    AggregateFailed { source: BlstError },
    #[doom(description("Incorrect buffer size"))]
    IncorrectBufferSize,
    #[doom(description("Malformed public key: {}", source))]
    #[doom(wrap(malformed_public_key))]
    MalformedPublicKey { source: BlstError },
    #[doom(description("Malformed secret key: {}", source))]
    #[doom(wrap(malformed_secret_key))]
    MalformedSecretKey { source: BlstError },
    #[doom(description("Malformed signature: {}", source))]
    #[doom(wrap(malformed_signature))]
    MalformedSignature { source: BlstError },
    #[doom(description("Failed to serialize: {}", source))]
    #[doom(wrap(serialize_failed))]
    SerializeFailed { source: bincode::Error },
    #[doom(description("Failed to `verify` signature: {}", source))]
    #[doom(wrap(verify_failed))]
    VerifyFailed { source: BlstError },
}

impl KeyPair {
    pub fn random() -> Self {
        KeyPair::from_rng(&mut OsRng)
    }

    pub fn from_rng<R>(rng: &mut R) -> Self
    where
        R: CryptoRng + RngCore,
    {
        let mut seed = [0; 32];
        rng.fill_bytes(&mut seed);

        let secret = BlstSecretKey::key_gen(&seed, &[]).unwrap();
        let public = secret.sk_to_pk();

        KeyPair { public, secret }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Top<MultiError>> {
        if bytes.len() != KEYPAIR_LENGTH {
            return MultiError::IncorrectBufferSize.fail().spot(here!());
        }

        let (public_bytes, secret_bytes) = bytes.split_at(PUBLIC_KEY_LENGTH);

        let public = BlstPublicKey::from_bytes(public_bytes)
            .map_err(Into::<BlstError>::into)
            .map_err(MultiError::malformed_public_key)
            .map_err(Doom::into_top)
            .spot(here!())?;

        let secret = BlstSecretKey::from_bytes(secret_bytes)
            .map_err(Into::<BlstError>::into)
            .map_err(MultiError::malformed_secret_key)
            .map_err(Doom::into_top)
            .spot(here!())?;

        Ok(KeyPair { public, secret })
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.public)
    }

    pub fn to_bytes(&self) -> [u8; KEYPAIR_LENGTH] {
        let mut keypair_bytes = [0u8; KEYPAIR_LENGTH];
        let (public_bytes, secret_bytes) = keypair_bytes.split_at_mut(PUBLIC_KEY_LENGTH);

        public_bytes.copy_from_slice(&self.public.serialize());
        secret_bytes.copy_from_slice(&self.secret.to_bytes());

        keypair_bytes
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
    /// use talk::crypto::primitives::multi::{Signature, KeyPair, PublicKey};
    ///
    /// let alice = KeyPair::random();
    /// let bob = KeyPair::random();
    ///
    /// let message: u32 = 1234;
    ///
    /// let alice_signature = alice.sign_raw(&message).unwrap();
    ///
    /// assert!(alice_signature.verify_raw(
    ///     [&alice.public()],
    ///     &message
    /// ).is_ok());
    /// ```
    pub fn sign_raw<T>(&self, message: &T) -> Result<Signature, Top<MultiError>>
    where
        T: Serialize,
    {
        let message = bincode::serialize(message)
            .map_err(MultiError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        let signature = self.secret.sign(&message, BLST_DST, &[]);
        Ok(Signature(signature))
    }
}

impl PublicKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Top<MultiError>> {
        if bytes.len() != PUBLIC_KEY_LENGTH {
            return MultiError::IncorrectBufferSize.fail().spot(here!());
        }

        let public_key = BlstPublicKey::from_bytes(bytes)
            .map_err(Into::<BlstError>::into)
            .map_err(MultiError::malformed_public_key)
            .map_err(Doom::into_top)
            .spot(here!())?;

        Ok(PublicKey(public_key))
    }

    pub fn to_bytes(&self) -> [u8; PUBLIC_KEY_LENGTH] {
        self.0.serialize()
    }
}

impl Signature {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Top<MultiError>> {
        if bytes.len() != SIGNATURE_LENGTH {
            return MultiError::IncorrectBufferSize.fail().spot(here!());
        }

        let signature = BlstSignature::from_bytes(bytes)
            .map_err(Into::<BlstError>::into)
            .map_err(MultiError::malformed_signature)
            .map_err(Doom::into_top)
            .spot(here!())?;

        Ok(Signature(signature))
    }

    pub fn to_bytes(&self) -> [u8; SIGNATURE_LENGTH] {
        self.0.serialize()
    }

    /// Aggregates a set of `Signature`s into a single `Signature`
    ///
    /// # Errors
    ///
    /// If the aggregation fails for any reason, an `AggregateFailed`
    /// error variant will be returned.
    ///
    /// # Examples
    /// ```
    /// use talk::crypto::primitives::multi::{Signature, KeyPair};
    ///
    /// let alice = KeyPair::random();
    /// let bob = KeyPair::random();
    /// let carl = KeyPair::random();
    ///
    /// let message: u32 = 1234;
    ///
    /// let alice_signature = alice.sign_raw(&message).unwrap();
    /// let bob_signature = bob.sign_raw(&message).unwrap();
    /// let carl_signature = carl.sign_raw(&message).unwrap();
    ///
    /// let signature = Signature::aggregate([
    ///     alice_signature,
    ///     bob_signature,
    ///     carl_signature,
    /// ]);
    ///
    /// assert!(signature.is_ok());
    /// ```
    pub fn aggregate<I>(signatures: I) -> Result<Self, Top<MultiError>>
    where
        I: IntoIterator<Item = Signature>,
    {
        let signatures = signatures.into_iter().collect::<Vec<_>>();

        let signatures = signatures
            .iter()
            .map(|signature| &signature.0)
            .collect::<Vec<_>>();

        let signature = BlstAggregateSignature::aggregate(&signatures[..], true)
            .map_err(Into::<BlstError>::into)
            .map_err(MultiError::aggregate_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        let signature = signature.to_signature();
        Ok(Signature(signature))
    }

    /// Verifies the `Signature` of a message against a set of `PublicKey`s.
    ///
    /// Verification succeeds if and only (i) for every PublicKey, the message
    /// is signed using its matching PrivateKey and (ii) the `Signature` is the
    /// aggregate of (and only of) those individual signatures.
    ///
    /// # Errors
    ///
    /// If the serialization of the message fails, a `SerializeFailed`
    /// error variant will be returned. If the verification fails for
    /// any reason, `VerifyFailed` will be returned.
    ///
    /// # Examples
    /// ```
    /// use talk::crypto::primitives::multi::{Signature, KeyPair, PublicKey};
    ///
    /// let alice = KeyPair::random();
    /// let bob = KeyPair::random();
    ///
    /// let message: u32 = 1234;
    ///
    /// let alice_signature = alice.sign_raw(&message).unwrap();
    /// let bob_signature = bob.sign_raw(&message).unwrap();
    ///
    /// let signature = Signature::aggregate([
    ///     alice_signature,
    ///     bob_signature,
    /// ])
    /// .unwrap();
    ///
    /// assert!(signature.verify_raw(
    ///     [&alice.public(), &bob.public()],
    ///     &message,
    /// ).is_ok());
    /// ```
    pub fn verify_raw<'p, P, M>(&self, signers: P, message: &M) -> Result<(), Top<MultiError>>
    where
        P: IntoIterator<Item = &'p PublicKey>,
        M: Serialize,
    {
        let signers = signers
            .into_iter()
            .map(|signer| &signer.0)
            .collect::<Vec<_>>();

        let message = bincode::serialize(message)
            .map_err(MultiError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        self.0
            .fast_aggregate_verify(true, &message[..], BLST_DST, &signers[..])
            .into_result()
            .map_err(MultiError::verify_failed)
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
        self.to_bytes().hash(state)
    }
}

impl Hash for Signature {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.to_bytes().hash(state)
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

impl Serialize for KeyPair {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.to_bytes())
    }
}

impl<'de> Deserialize<'de> for KeyPair {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};

        struct ByteVisitor;

        impl<'de> Visitor<'de> for ByteVisitor {
            type Value = KeyPair;

            fn expecting(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
                f.write_str(
                    "byte representation of a bls keypair (concatenated public and secret key)",
                )
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                KeyPair::from_bytes(v).map_err(E::custom)
            }
        }

        deserializer.deserialize_bytes(ByteVisitor)
    }
}

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.to_bytes())
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};

        struct ByteVisitor;

        impl<'de> Visitor<'de> for ByteVisitor {
            type Value = PublicKey;

            fn expecting(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
                f.write_str("byte representation of a bls public key")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                PublicKey::from_bytes(v).map_err(E::custom)
            }
        }

        deserializer.deserialize_bytes(ByteVisitor)
    }
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.to_bytes())
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};

        struct ByteVisitor;

        impl<'de> Visitor<'de> for ByteVisitor {
            type Value = Signature;

            fn expecting(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
                f.write_str("byte representation of a bls public key")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Signature::from_bytes(v).map_err(E::custom)
            }
        }

        deserializer.deserialize_bytes(ByteVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_correct() {
        let keypair = KeyPair::random();
        let message: u32 = 1234;
        let signature = keypair.sign_raw(&message).unwrap();

        signature.verify_raw([&keypair.public()], &message).unwrap();
    }

    #[test]
    fn single_compromise_message() {
        let keypair = KeyPair::random();
        let message: u32 = 1234;
        let signature = keypair.sign_raw(&message).unwrap();

        let message: u32 = 1235;

        assert!(signature.verify_raw([&keypair.public()], &message).is_err());
    }

    #[test]
    fn single_compromise_signature() {
        let keypair = KeyPair::random();
        let message: u32 = 1234;
        let signature = keypair.sign_raw(&message).unwrap();

        let mut signature = bincode::serialize(&signature).unwrap();
        signature[10] = signature[10].wrapping_add(1);
        let signature: Result<Signature, _> = bincode::deserialize(&signature);

        if let Ok(signature) = signature {
            // Sometimes at random, deserializing a tampered signature results itself in an `Err`
            assert!(signature
                .verify_raw([&keypair.public()], &message,)
                .is_err());
        }
    }

    #[test]
    fn multiple_correct() {
        let alice = KeyPair::random();
        let bob = KeyPair::random();
        let carl = KeyPair::random();

        let message: u32 = 1234;

        let alice_signature = alice.sign_raw(&message).unwrap();
        let bob_signature = bob.sign_raw(&message).unwrap();
        let carl_signature = carl.sign_raw(&message).unwrap();

        let signature =
            Signature::aggregate([alice_signature, bob_signature, carl_signature]).unwrap();

        signature
            .verify_raw([&alice.public(), &bob.public(), &carl.public()], &message)
            .unwrap();
    }

    #[test]
    fn multiple_compromise_message() {
        let alice = KeyPair::random();
        let bob = KeyPair::random();
        let carl = KeyPair::random();

        let message: u32 = 1234;

        let alice_signature = alice.sign_raw(&message).unwrap();
        let bob_signature = bob.sign_raw(&message).unwrap();
        let carl_signature = carl.sign_raw(&message).unwrap();

        let signature =
            Signature::aggregate([alice_signature, bob_signature, carl_signature]).unwrap();

        let message: u32 = 1235;

        assert!(signature
            .verify_raw([&alice.public(), &bob.public(), &carl.public()], &message,)
            .is_err());
    }

    #[test]
    fn multiple_compromise_signature() {
        let alice = KeyPair::random();
        let bob = KeyPair::random();
        let carl = KeyPair::random();

        let message: u32 = 1234;

        let alice_signature = alice.sign_raw(&message).unwrap();
        let bob_signature = bob.sign_raw(&message).unwrap();
        let carl_signature = carl.sign_raw(&message).unwrap();

        let signature =
            Signature::aggregate([alice_signature, bob_signature, carl_signature]).unwrap();

        let mut signature = bincode::serialize(&signature).unwrap();
        signature[10] = signature[10].wrapping_add(1);
        let signature: Result<Signature, _> = bincode::deserialize(&signature);

        if let Ok(signature) = signature {
            // Sometimes at random, deserializing a tampered signature results itself in an `Err`
            assert!(signature
                .verify_raw([&alice.public(), &bob.public(), &carl.public()], &message,)
                .is_err());
        }
    }

    #[test]
    fn serialize_keypair() {
        let original = KeyPair::random();
        let serialized = bincode::serialize(&original).unwrap();
        let deserialized = bincode::deserialize::<KeyPair>(serialized.as_slice()).unwrap();

        assert_eq!(original.public.to_bytes(), deserialized.public.to_bytes());
        assert_eq!(original.secret.to_bytes(), deserialized.secret.to_bytes());

        let message = 42u64;
        let signature = deserialized.sign_raw(&message).unwrap();
        signature
            .verify_raw([&original.public()], &message)
            .unwrap();
    }
}
