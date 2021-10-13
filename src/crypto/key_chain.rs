use crate::crypto::{
    primitives::{
        multi::{
            KeyPair as MultiKeyPair, MultiError, Signature as MultiSignature,
        },
        sign::{KeyPair as SignKeyPair, SignError, Signature as SignSignature},
    },
    KeyCard, Statement,
};

use doomstack::Top;

use std::sync::Arc;

#[derive(Clone)]
pub struct KeyChain {
    pub(in crate::crypto) keypairs: Arc<KeyPairs>,
}

pub(in crate::crypto) struct KeyPairs {
    pub(in crate::crypto) sign: SignKeyPair,
    pub(in crate::crypto) multi: MultiKeyPair,
}

impl KeyChain {
    pub fn random() -> Self {
        let keypairs = Arc::new(KeyPairs {
            sign: SignKeyPair::random(),
            multi: MultiKeyPair::random(),
        });

        KeyChain { keypairs }
    }

    pub fn keycard(&self) -> KeyCard {
        KeyCard::from_keychain(&self)
    }

    pub fn sign<S: Statement>(
        &self,
        message: &S,
    ) -> Result<SignSignature, Top<SignError>> {
        self.keypairs.sign.sign_raw(&(S::SCOPE, S::HEADER, message))
    }

    pub fn multisign<S: Statement>(
        &self,
        message: &S,
    ) -> Result<MultiSignature, Top<MultiError>> {
        self.keypairs
            .multi
            .sign_raw(&(S::SCOPE, S::HEADER, message))
    }
}
