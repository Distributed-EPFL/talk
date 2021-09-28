use crate::crypto::primitives::{
    adapters::BlstErrorAdapter,
    errors::{
        multi::{AggregateFailed, BlstError, SerializeFailed, VerifyFailed},
        MultiError,
    },
};

use blst::min_sig::{
    AggregateSignature as BlstAggregateSignature, PublicKey as BlstPublicKey,
    SecretKey as BlstSecretKey, Signature as BlstSignature,
};

use rand::rngs::OsRng;
use rand::RngCore;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use snafu::ResultExt;

use std::hash::{Hash, Hasher};

pub const KEYPAIR_LENGTH: usize = 128;
pub const PUBLIC_KEY_LENGTH: usize = 96;
pub const SIGNATURE_LENGTH: usize = 48;

const BLST_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";

pub struct KeyPair {
    public: BlstPublicKey,
    secret: BlstSecretKey,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PublicKey(BlstPublicKey);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature(BlstSignature);

impl KeyPair {
    pub fn random() -> Self {
        let mut seed = [0; 32];
        OsRng.fill_bytes(&mut seed);

        let secret = BlstSecretKey::key_gen(&seed, &[]).unwrap();
        let public = secret.sk_to_pk();

        KeyPair { public, secret }
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.public)
    }

    pub fn sign_raw<T>(&self, message: &T) -> Result<Signature, MultiError>
    where
        T: Serialize,
    {
        let message = bincode::serialize(message).context(SerializeFailed)?;
        let signature = self.secret.sign(&message, BLST_DST, &[]);
        Ok(Signature(signature))
    }
}

impl PublicKey {
    pub fn to_bytes(&self) -> [u8; PUBLIC_KEY_LENGTH] {
        self.0.to_bytes()
    }

    pub fn verify_raw<'p, P, M>(
        signers: P,
        message: &M,
        signature: &Signature,
    ) -> Result<(), MultiError>
    where
        P: IntoIterator<Item = &'p PublicKey>,
        M: Serialize,
    {
        let signers = signers
            .into_iter()
            .map(|signer| &signer.0)
            .collect::<Vec<_>>();

        let message = bincode::serialize(message).context(SerializeFailed)?;

        signature
            .0
            .fast_aggregate_verify(true, &message[..], BLST_DST, &signers[..])
            .into_result()
            .context(VerifyFailed)
    }
}

impl Signature {
    pub fn to_bytes(&self) -> [u8; SIGNATURE_LENGTH] {
        self.0.to_bytes()
    }

    pub fn aggregate<I>(signatures: I) -> Result<Self, MultiError>
    where
        I: IntoIterator<Item = Signature>,
    {
        let signatures = signatures.into_iter().collect::<Vec<_>>();

        let signatures = signatures
            .iter()
            .map(|signature| &signature.0)
            .collect::<Vec<_>>();

        let signature =
            BlstAggregateSignature::aggregate(&signatures[..], true)
                .map_err(Into::into)
                .context(AggregateFailed)?;

        let signature = signature.to_signature();
        Ok(Signature(signature))
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

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0.to_bytes())
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};
        use std::fmt::{Formatter, Result as FmtResult};

        struct ByteVisitor;

        impl<'de> Visitor<'de> for ByteVisitor {
            type Value = BlstPublicKey;

            fn expecting(&self, f: &mut Formatter) -> FmtResult {
                f.write_str("byte representation of a bls public key")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                BlstPublicKey::from_bytes(v)
                    .map_err(Into::<BlstError>::into)
                    .map_err(E::custom)
            }
        }

        Ok(Self(deserializer.deserialize_bytes(ByteVisitor)?))
    }
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0.to_bytes())
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};
        use std::fmt::{Formatter, Result as FmtResult};

        struct ByteVisitor;

        impl<'de> Visitor<'de> for ByteVisitor {
            type Value = BlstSignature;

            fn expecting(&self, f: &mut Formatter) -> FmtResult {
                f.write_str("byte representation of a bls public key")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                BlstSignature::from_bytes(v)
                    .map_err(Into::<BlstError>::into)
                    .map_err(E::custom)
            }
        }

        Ok(Self(deserializer.deserialize_bytes(ByteVisitor)?))
    }
}
