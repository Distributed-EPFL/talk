use crate::crypto::{
    primitives::{
        errors::{MultiError, SignError},
        multi::{KeyPair as MultiKeyPair, Signature as MultiSignature},
        sign::{KeyPair as SignKeyPair, Signature as SignSignature},
    },
    KeyCard, Statement,
};

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
    ) -> Result<SignSignature, SignError> {
        self.keypairs.sign.sign_raw(&(S::SCOPE, S::HEADER, message))
    }

    pub fn multisign<S: Statement>(
        &self,
        message: &S,
    ) -> Result<MultiSignature, MultiError> {
        self.keypairs
            .multi
            .sign_raw(&(S::SCOPE, S::HEADER, message))
    }
}
