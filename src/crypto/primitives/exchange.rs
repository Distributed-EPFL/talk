use rand::rngs::OsRng;

use serde::{Deserialize, Serialize};

use std::fmt::{Debug, Error as FmtError, Formatter};

use x25519_dalek::{
    EphemeralSecret as XEphemeralSecret, PublicKey as XPublicKey,
    SharedSecret as XSharedSecret,
};

pub struct KeyPair {
    public: XPublicKey,
    secret: XEphemeralSecret,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicKey(XPublicKey);

pub struct SharedKey(XSharedSecret);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Role {
    Even = 0,
    Odd = 1,
}

pub const PUBLIC_KEY_LENGTH: usize = 32;
pub const SHARED_KEY_LENGTH: usize = 32;

impl KeyPair {
    pub fn random() -> Self {
        let secret = XEphemeralSecret::new(OsRng);
        let public = XPublicKey::from(&secret);

        KeyPair { public, secret }
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.public)
    }

    pub fn exchange(self, remote: PublicKey) -> (SharedKey, Role) {
        let shared_key = SharedKey(self.secret.diffie_hellman(&remote.0));

        // If `self.public.to_bytes() == remote.to_bytes()`, then a
        // remote host maliciously echoed `self.public`, and will not
        // be able to decrypt or authenticate messages. In any case,
        // communication is compromised and potentially leaked.
        let role = if self.public.to_bytes() > remote.to_bytes() {
            Role::Even
        } else {
            Role::Odd
        };

        (shared_key, role)
    }
}

impl PublicKey {
    fn from_bytes(bytes: [u8; PUBLIC_KEY_LENGTH]) -> Self {
        PublicKey(bytes.into())
    }

    fn to_bytes(&self) -> [u8; PUBLIC_KEY_LENGTH] {
        self.0.to_bytes()
    }
}

impl Debug for PublicKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
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

impl SharedKey {
    pub(in crate::crypto::primitives) fn to_bytes(
        &self,
    ) -> [u8; SHARED_KEY_LENGTH] {
        self.0.to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct() {
        let alice_keypair = KeyPair::random();
        let bob_keypair = KeyPair::random();

        let alice_public = alice_keypair.public();
        let bob_public = bob_keypair.public();

        let (alice_shared, alice_role) = alice_keypair.exchange(bob_public);
        let (bob_shared, bob_role) = bob_keypair.exchange(alice_public);

        assert_eq!(alice_shared.to_bytes(), bob_shared.to_bytes());
        assert_ne!(alice_role, bob_role);
    }

    #[test]
    fn compromise_public_keys() {
        let alice_keypair = KeyPair::random();
        let bob_keypair = KeyPair::random();

        let alice_public = alice_keypair.public();
        let bob_public = bob_keypair.public();

        let mut alice_public = alice_public.to_bytes();
        alice_public[3] = alice_public[3].wrapping_add(1);
        let alice_public = PublicKey::from_bytes(alice_public);

        let mut bob_public = bob_public.to_bytes();
        bob_public[3] = bob_public[3].wrapping_add(1);
        let bob_public = PublicKey::from_bytes(bob_public);

        let (alice_shared, _) = alice_keypair.exchange(bob_public);
        let (bob_shared, _) = bob_keypair.exchange(alice_public);

        assert_ne!(alice_shared.to_bytes(), bob_shared.to_bytes());
    }

    #[test]
    fn man_in_the_middle() {
        let alice_keypair = KeyPair::random();
        let eve_keypair = KeyPair::random();
        let bob_keypair = KeyPair::random();

        let eve_public = eve_keypair.public();

        let (alice_shared, _) = alice_keypair.exchange(eve_public);
        let (bob_shared, _) = bob_keypair.exchange(eve_public);

        assert_ne!(alice_shared.to_bytes(), bob_shared.to_bytes());
    }
}
