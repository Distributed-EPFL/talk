use crate::crypto::{
    primitives::{
        hash,
        multi::{MultiError, PublicKey as MultiPublicKey, Signature as MultiSignature},
        sign::{PublicKey as SignPublicKey, SignError, Signature as SignSignature},
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
        let keys = PublicKeys {
            sign: keychain.keypairs.sign.public(),
            multi: keychain.keypairs.multi.public(),
        };

        KeyCard::from_keys(keys)
    }

    fn from_keys(keys: PublicKeys) -> Self {
        let identity = Identity::from_hash(hash::hash(&keys).unwrap());
        KeyCard { identity, keys }
    }

    pub fn identity(&self) -> Identity {
        self.identity
    }
}

impl SignSignature {
    pub fn verify<S>(&self, keycard: &KeyCard, message: &S) -> Result<(), Top<SignError>>
    where
        S: Statement,
    {
        self.verify_raw(keycard.keys.sign, &(S::SCOPE, S::HEADER, message))
    }
}

impl MultiSignature {
    pub fn verify<'c, C, S>(&self, cards: C, message: &S) -> Result<(), Top<MultiError>>
    where
        C: IntoIterator<Item = &'c KeyCard>,
        S: Statement,
    {
        self.verify_raw(
            cards.into_iter().map(|card| &card.keys.multi),
            &(S::SCOPE, S::HEADER, message),
        )
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
        Ok(KeyCard::from_keys(keys))
    }
}
