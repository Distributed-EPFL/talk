use crate::crypto::{
    primitives::{
        hash,
        multi::{
            MultiError, PublicKey as MultiPublicKey, Signature as MultiSignature,
            Signer as MultiSigner,
        },
        sign::{
            PublicKey as SignPublicKey, SignError, Signature as SignSignature, Signer as SignSigner,
        },
    },
    Identity, KeyChain, Statement,
};

use doomstack::Top;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::{
    cmp::{Ord, Ordering, PartialOrd},
    hash::{Hash, Hasher},
};

#[derive(Debug, Clone)]
pub struct KeyCard {
    identity: Identity,
    keys: PublicKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PublicKeys {
    sign: SignPublicKey,
    multi: MultiPublicKey,
}

impl KeyCard {
    pub fn from_keychain(keychain: &KeyChain) -> Self {
        KeyCard::from_public_keys(
            keychain.keypairs.sign.public(),
            keychain.keypairs.multi.public(),
        )
    }

    pub fn from_public_keys(sign: SignPublicKey, multi: MultiPublicKey) -> Self {
        let keys = PublicKeys { sign, multi };
        let identity = Identity::from_hash(hash::hash(&keys).unwrap());

        KeyCard { identity, keys }
    }

    pub fn identity(&self) -> Identity {
        self.identity
    }
}

impl SignSignature {
    pub fn verify<R, S>(&self, signer: &R, message: &S) -> Result<(), Top<SignError>>
    where
        R: SignSigner,
        S: Statement,
    {
        self.verify_raw(signer.public_key(), &(S::SCOPE, S::HEADER, message))
    }

    pub fn batch_verify<'a, RI, R, MI, M, SI>(
        signers: RI,
        messages: MI,
        signatures: SI,
    ) -> Result<(), Top<SignError>>
    where
        RI: IntoIterator<Item = &'a R>,
        R: 'a + SignSigner,
        MI: IntoIterator<Item = &'a M>,
        M: 'a + Statement,
        SI: IntoIterator<Item = &'a SignSignature>,
    {
        let public_keys = signers
            .into_iter()
            .map(SignSigner::public_key)
            .copied()
            .collect::<Vec<_>>();

        let messages = messages
            .into_iter()
            .map(|message| (M::SCOPE, M::HEADER, message))
            .collect::<Vec<_>>();

        let messages = messages.iter();

        let signatures = signatures.into_iter().copied();

        SignSignature::batch_verify_raw(public_keys, messages, signatures)
    }
}

impl MultiSignature {
    pub fn verify<'c, RI, R, S>(&self, signers: RI, message: &S) -> Result<(), Top<MultiError>>
    where
        RI: IntoIterator<Item = &'c R>,
        R: 'c + MultiSigner,
        S: Statement,
    {
        self.verify_raw(
            signers.into_iter().map(|signer| signer.public_key()),
            &(S::SCOPE, S::HEADER, message),
        )
    }
}

impl SignSigner for KeyCard {
    fn public_key(&self) -> &SignPublicKey {
        &self.keys.sign
    }
}

impl MultiSigner for KeyCard {
    fn public_key(&self) -> &MultiPublicKey {
        &self.keys.multi
    }
}

impl PartialEq for KeyCard {
    fn eq(&self, rho: &KeyCard) -> bool {
        self.identity == rho.identity
    }
}

impl Eq for KeyCard {}

impl PartialOrd for KeyCard {
    fn partial_cmp(&self, rho: &Self) -> Option<Ordering> {
        Some(self.cmp(&rho))
    }
}

impl Ord for KeyCard {
    fn cmp(&self, rho: &Self) -> Ordering {
        self.identity.cmp(&rho.identity)
    }
}

impl Hash for KeyCard {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.identity.hash(state)
    }
}

impl Serialize for KeyCard {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.keys.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for KeyCard {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let keys = PublicKeys::deserialize(deserializer)?;
        Ok(KeyCard::from_public_keys(keys.sign, keys.multi))
    }
}
