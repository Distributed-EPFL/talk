use crate::crypto::{
    primitives::{
        multi::{
            MultiError, PublicKey as MultiPublicKey,
            Signature as MultiSignature,
        },
        sign::{
            PublicKey as SignPublicKey, SignError, Signature as SignSignature,
        },
    },
    KeyChain, Statement,
};

use doomstack::Top;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyCard {
    sign: SignPublicKey,
    multi: MultiPublicKey,
}

impl KeyCard {
    pub fn from_keychain(keychain: &KeyChain) -> Self {
        KeyCard {
            sign: keychain.keypairs.sign.public(),
            multi: keychain.keypairs.multi.public(),
        }
    }

    pub fn root(&self) -> SignPublicKey {
        self.sign
    }
}

impl SignSignature {
    pub fn verify<S>(
        &self,
        keycard: &KeyCard,
        message: &S,
    ) -> Result<(), Top<SignError>>
    where
        S: Statement,
    {
        self.verify_raw(keycard.sign, &(S::SCOPE, S::HEADER, message))
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
            cards.into_iter().map(|card| card.multi),
            &(S::SCOPE, S::HEADER, message),
        )
    }
}
