use crate::crypto::{
    errors::{KeyCardError, MalformedKeyCard},
    primitives::{
        errors::SignError,
        multi::{
            MultiError, PublicKey as MultiPublicKey,
            Signature as MultiSignature,
        },
        sign::{PublicKey as SignPublicKey, Signature as SignSignature},
    },
    KeyChain, Scope, Statement, TalkHeader,
};

use doomstack::Top;

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use snafu::ResultExt;

#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub struct KeyCard {
    keys: PublicKeys,
    signature: SignSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

        let signature = keychain.sign(&keys).unwrap();

        KeyCard { keys, signature }
    }

    pub fn root(&self) -> SignPublicKey {
        self.keys.sign
    }

    fn check(&self) -> Result<(), KeyCardError> {
        self.signature
            .verify(&self, &self.keys)
            .context(MalformedKeyCard)
    }
}

impl SignSignature {
    pub fn verify<S>(
        &self,
        keycard: &KeyCard,
        message: &S,
    ) -> Result<(), SignError>
    where
        S: Statement,
    {
        self.verify_raw(keycard.keys.sign, &(S::SCOPE, S::HEADER, message))
    }
}

impl MultiSignature {
    pub fn verify<'c, C, S>(
        &self,
        cards: C,
        message: &S,
    ) -> Result<(), Top<MultiError>>
    where
        C: IntoIterator<Item = &'c KeyCard>,
        S: Statement,
    {
        self.verify_raw(
            cards.into_iter().map(|card| card.keys.multi),
            &(S::SCOPE, S::HEADER, message),
        )
    }
}

impl PartialEq for KeyCard {
    fn eq(&self, rho: &KeyCard) -> bool {
        self.keys == rho.keys
    }
}

impl Serialize for KeyCard {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        KeyCard::serialize(&self, serializer)
    }
}

impl<'de> Deserialize<'de> for KeyCard {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let card = KeyCard::deserialize(deserializer)?;
        card.check().map_err(|err| DeError::custom(err))?;
        Ok(card)
    }
}

impl Statement for PublicKeys {
    const SCOPE: Scope = Scope::talk();
    type Header = TalkHeader;
    const HEADER: TalkHeader = TalkHeader::KeyCardPublicKeys;
}
