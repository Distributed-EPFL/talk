use crate::crypto::{
    primitives::{
        errors::{MultiError, SignError},
        multi::{KeyPair as MultiKeyPair, Signature as MultiSignature},
        sign::{KeyPair as SignKeyPair, Signature as SignSignature},
    },
    KeyCard, Statement,
};

pub struct KeyChain {
    pub(in crate::crypto) sign: SignKeyPair,
    pub(in crate::crypto) multi: MultiKeyPair,
}

impl KeyChain {
    pub fn random() -> Self {
        KeyChain {
            sign: SignKeyPair::random(),
            multi: MultiKeyPair::random(),
        }
    }

    pub fn keycard(&self) -> KeyCard {
        KeyCard::from_keychain(&self)
    }

    pub fn sign<S: Statement>(
        &self,
        message: &S,
    ) -> Result<SignSignature, SignError> {
        self.sign.sign_raw(&(S::SCOPE, S::HEADER, message))
    }

    pub fn multisign<S: Statement>(
        &self,
        message: &S,
    ) -> Result<MultiSignature, MultiError> {
        self.multi.sign_raw(&(S::SCOPE, S::HEADER, message))
    }
}
