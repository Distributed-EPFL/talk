use blake3::{Hash, Hasher};

use chacha20poly1305::aead::{
    Aead as ChaChaAead, AeadInPlace as ChaChaAeadInPlace,
    NewAead as ChaChaNewAead,
};
use chacha20poly1305::{
    ChaCha20Poly1305, Key as ChaChaKey, Nonce as ChaChaNonce,
};

use crate::crypto::primitives::{
    errors::{
        channel::{AuthenticateFailed, DeserializeFailed, SerializeFailed},
        ChannelError,
    },
    exchange::{Role, SharedKey},
    hash::HASH_LENGTH,
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
    hasher: Hasher,
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
            .encrypt_in_place(
                &ChaChaNonce::from_slice(&nonce),
                &[],
                buffer as &mut Vec<u8>,
            )
            .unwrap();

        Ok(())
    }

    pub fn authenticate<M>(
        &mut self,
        message: &M,
    ) -> Result<Vec<u8>, ChannelError>
    where
        M: Serialize,
    {
        let mut buffer = Vec::new();
        self.authenticate_into(message, &mut buffer)?;
        Ok(buffer)
    }

    pub fn authenticate_into<M>(
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

        self.0.hasher.reset();
        self.0.hasher.update(&nonce);
        self.0.hasher.update(&buffer);

        let tag = self.0.hasher.finalize();
        buffer.extend_from_slice(tag.as_bytes());

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
            .decrypt(&ChaChaNonce::from_slice(&nonce), message)
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
            .decrypt_in_place(
                &ChaChaNonce::from_slice(&nonce),
                &[],
                message as &mut Vec<u8>,
            )
            .map_err(|_| ChannelError::DecryptFailed)?;

        bincode::deserialize(message).context(DeserializeFailed)
    }

    pub fn authenticate<M>(&mut self, message: &[u8]) -> Result<M, ChannelError>
    where
        M: for<'de> Deserialize<'de>,
    {
        let nonce = self.0.nonce();

        if message.len() < HASH_LENGTH {
            AuthenticateFailed.fail()
        } else {
            let (message, tag) = message.split_at(message.len() - HASH_LENGTH);

            let tag: [u8; HASH_LENGTH] = tag.try_into().unwrap(); // This is guaranteed to work because `message.len() >= HASH_LENGTH`
            let tag: Hash = tag.into();

            self.0.hasher.reset();
            self.0.hasher.update(&nonce);
            self.0.hasher.update(message);

            let digest = self.0.hasher.finalize();

            if tag == digest {
                bincode::deserialize(message).context(DeserializeFailed)
            } else {
                AuthenticateFailed.fail()
            }
        }
    }
}

impl State {
    fn nonce(&mut self) -> [u8; NONCE_LENGTH] {
        let mut nonce: [u8; NONCE_LENGTH] = self.nonce.to_be_bytes()
            [16 - NONCE_LENGTH..]
            .try_into()
            .unwrap();

        nonce[0] = self.lane as u8;
        self.nonce += 1;

        nonce
    }
}

pub fn channel(key: SharedKey, role: Role) -> (Sender, Receiver) {
    let key = key.to_bytes();

    let cipher_key = ChaChaKey::from_slice(&key);
    let hasher_key = key;

    let (sender_lane, receiver_lane) = match role {
        Role::Even => (Lane::High, Lane::Low),
        Role::Odd => (Lane::Low, Lane::High),
    };

    let sender = Sender(State {
        cipher: ChaCha20Poly1305::new(cipher_key),
        hasher: Hasher::new_keyed(&hasher_key),
        lane: sender_lane,
        nonce: 0,
    });

    let receiver = Receiver(State {
        cipher: ChaCha20Poly1305::new(cipher_key),
        hasher: Hasher::new_keyed(&hasher_key),
        lane: receiver_lane,
        nonce: 0,
    });

    (sender, receiver)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::crypto::primitives::exchange::KeyPair;

    fn setup() -> ((Sender, Receiver), (Sender, Receiver)) {
        let alice_keypair = KeyPair::random();
        let bob_keypair = KeyPair::random();

        let alice_public_key = alice_keypair.public();
        let bob_public_key = bob_keypair.public();

        let (alice_shared_key, alice_role) =
            alice_keypair.exchange(bob_public_key);

        let (bob_shared_key, bob_role) = bob_keypair.exchange(alice_public_key);

        let alice_channel = channel(alice_shared_key, alice_role);

        let bob_channel = channel(bob_shared_key, bob_role);

        (alice_channel, bob_channel)
    }

    #[test]
    fn encrypt_correct() {
        let (
            (mut alice_sender, mut alice_receiver),
            (mut bob_sender, mut bob_receiver),
        ) = setup();

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

    #[test]
    fn encrypt_compromise() {
        let ((mut alice_sender, _), (_, mut bob_receiver)) = setup();

        let mut ciphertext = alice_sender.encrypt(&33u32).unwrap();
        ciphertext[3] = ciphertext[3].wrapping_add(1);

        assert!(bob_receiver.decrypt::<u32>(&ciphertext[..]).is_err());
    }

    #[test]
    fn encrypt_compromise_then_correct() {
        let ((mut alice_sender, _), (_, mut bob_receiver)) = setup();

        let mut ciphertext = alice_sender.encrypt(&33u32).unwrap();
        ciphertext[3] = ciphertext[3].wrapping_add(1);

        let _ = bob_receiver.decrypt::<u32>(&ciphertext[..]);

        let ciphertext = alice_sender.encrypt(&34u32).unwrap();
        let plaintext: u32 = bob_receiver.decrypt(&ciphertext[..]).unwrap();

        assert_eq!(plaintext, 34u32);
    }
}
