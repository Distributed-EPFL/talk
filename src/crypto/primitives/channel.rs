use chacha20poly1305::aead::{
    Aead as ChaChaAead, AeadInPlace as ChaChaAeadInPlace,
    NewAead as ChaChaNewAead,
};
use chacha20poly1305::{
    ChaCha20Poly1305, Key as ChaChaKey, Nonce as ChaChaNonce,
};

use crate::crypto::primitives::{
    errors::{
        channel::{DeserializeFailed, SerializeFailed},
        ChannelError,
    },
    exchange::{Role, SharedKey},
};

use serde::{Deserialize, Serialize};

use snafu::ResultExt;

use std::convert::TryInto;

const NONCE_LENGTH: usize = 12;

pub struct Sender(State);
pub struct Receiver(State);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
enum Lane {
    Low = 0,
    High = 1,
}

struct State {
    cipher: ChaCha20Poly1305,
    lane: Lane,
    nonce: u128,
}

impl Sender {
    pub fn encrypt<M>(&mut self, message: &M) -> Result<Vec<u8>, ChannelError>
    where
        M: Serialize,
    {
        let mut buffer = Vec::new();
        self.encrypt_into(message, &mut buffer)?;
        Ok(buffer)
    }

    pub fn encrypt_into<M>(
        &mut self,
        message: &M,
        buffer: &mut Vec<u8>,
    ) -> Result<(), ChannelError>
    where
        M: Serialize,
    {
        bincode::serialize_into(buffer as &mut Vec<u8>, message)
            .context(SerializeFailed)?;

        let nonce = self.0.nonce();

        self.0
            .cipher
            .encrypt_in_place(&nonce, &[], buffer as &mut Vec<u8>)
            .unwrap();

        Ok(())
    }
}

impl Receiver {
    pub fn decrypt<M>(&mut self, message: &[u8]) -> Result<M, ChannelError>
    where
        M: for<'de> Deserialize<'de>,
    {
        let nonce = self.0.nonce();

        let message = self
            .0
            .cipher
            .decrypt(&nonce, message)
            .map_err(|_| ChannelError::DecryptFailed)?;

        bincode::deserialize(&message).context(DeserializeFailed)
    }

    pub fn decrypt_in_place<M>(
        &mut self,
        message: &mut Vec<u8>,
    ) -> Result<M, ChannelError>
    where
        M: for<'de> Deserialize<'de>,
    {
        let nonce = self.0.nonce();

        self.0
            .cipher
            .decrypt_in_place(&nonce, &[], message as &mut Vec<u8>)
            .map_err(|_| ChannelError::DecryptFailed)?;

        bincode::deserialize(message).context(DeserializeFailed)
    }
}

impl State {
    fn nonce(&mut self) -> ChaChaNonce {
        let mut nonce: [u8; NONCE_LENGTH] = self.nonce.to_be_bytes()
            [16 - NONCE_LENGTH..]
            .try_into()
            .unwrap();

        nonce[0] = self.lane as u8;
        self.nonce += 1;

        *ChaChaNonce::from_slice(&nonce)
    }
}

pub fn channel(key: SharedKey, role: Role) -> (Sender, Receiver) {
    let key = key.to_bytes();
    let key = ChaChaKey::from_slice(&key);

    let (sender_lane, receiver_lane) = match role {
        Role::Even => (Lane::High, Lane::Low),
        Role::Odd => (Lane::Low, Lane::High),
    };

    let sender = Sender(State {
        cipher: ChaCha20Poly1305::new(key),
        lane: sender_lane,
        nonce: 0,
    });

    let receiver = Receiver(State {
        cipher: ChaCha20Poly1305::new(key),
        lane: receiver_lane,
        nonce: 0,
    });

    (sender, receiver)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::crypto::primitives::exchange::KeyPair;

    #[test]
    fn correct() {
        let alice_keypair = KeyPair::random();
        let bob_keypair = KeyPair::random();

        let alice_public_key = alice_keypair.public();
        let bob_public_key = bob_keypair.public();

        let (alice_shared_key, alice_role) =
            alice_keypair.exchange(bob_public_key);
        let (bob_shared_key, bob_role) = bob_keypair.exchange(alice_public_key);

        let (mut alice_sender, mut alice_receiver) =
            channel(alice_shared_key, alice_role);
        let (mut bob_sender, mut bob_receiver) =
            channel(bob_shared_key, bob_role);

        for message in 0..128u32 {
            let ciphertext = alice_sender.encrypt(&message).unwrap();
            let plaintext: u32 = bob_receiver.decrypt(&ciphertext[..]).unwrap();
            assert_eq!(plaintext, message);

            let ciphertext = bob_sender.encrypt(&message).unwrap();
            let plaintext: u32 =
                alice_receiver.decrypt(&ciphertext[..]).unwrap();
            assert_eq!(plaintext, message);
        }
    }
}
