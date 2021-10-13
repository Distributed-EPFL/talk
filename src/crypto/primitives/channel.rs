use blake3::{Hash, Hasher};

use chacha20poly1305::aead::{
    Aead as ChaChaAead, AeadInPlace as ChaChaAeadInPlace,
    NewAead as ChaChaNewAead,
};
use chacha20poly1305::{
    ChaCha20Poly1305, Key as ChaChaKey, Nonce as ChaChaNonce,
};

use crate::crypto::primitives::{
    exchange::{Role, SharedKey},
    hash::HASH_LENGTH,
};

use serde::{Deserialize, Serialize};

use doomstack::{here, Doom, ResultExt, Top};

use std::convert::TryInto;

const NONCE_LENGTH: usize = 12;

pub struct Sender(State);
pub struct Receiver(State);

struct State {
    cipher: ChaCha20Poly1305,
    hasher: Hasher,
    lane: Lane,
    nonce: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
enum Lane {
    Low = 0,
    High = 1,
}

#[derive(Doom)]
pub enum ChannelError {
    #[doom(description("Failed to serialize: {:?}", source))]
    #[doom(wrap(serialize_failed))]
    SerializeFailed { source: bincode::Error },
    #[doom(description("Failed to deserialize: {:?}", source))]
    #[doom(wrap(deserialize_failed))]
    DeserializeFailed { source: bincode::Error },
    #[doom(description("Failed to `decrypt`"))]
    DecryptFailed,
    #[doom(description("Failed to `authenticate`"))]
    AuthenticateFailed,
}

impl Sender {
    pub fn encrypt<M>(
        &mut self,
        message: &M,
    ) -> Result<Vec<u8>, Top<ChannelError>>
    where
        M: Serialize,
    {
        let mut buffer = Vec::new(); // Create a `buffer` to encrypt into
        self.encrypt_into(message, &mut buffer)?; // Encrypt `message` into `buffer`
        Ok(buffer)
    }

    pub fn encrypt_into<M>(
        &mut self,
        message: &M,
        buffer: &mut Vec<u8>,
    ) -> Result<(), Top<ChannelError>>
    where
        M: Serialize,
    {
        bincode::serialize_into(buffer as &mut Vec<u8>, message)
            .map_err(ChannelError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?; // Serialize `message` into `buffer`

        let nonce = self.0.nonce(); // Generate a new `nonce`

        self.0
            .cipher
            .encrypt_in_place(
                &ChaChaNonce::from_slice(&nonce),
                &[],
                buffer as &mut Vec<u8>,
            )
            .unwrap(); // Encrypt `buffer` in place

        Ok(())
    }

    pub fn authenticate<M>(
        &mut self,
        message: &M,
    ) -> Result<Vec<u8>, Top<ChannelError>>
    where
        M: Serialize,
    {
        let mut buffer = Vec::new(); // Create a `buffer` to authenticate into
        self.authenticate_into(message, &mut buffer)?; // Authenticate `message` into `buffer`
        Ok(buffer)
    }

    pub fn authenticate_into<M>(
        &mut self,
        message: &M,
        buffer: &mut Vec<u8>,
    ) -> Result<(), Top<ChannelError>>
    where
        M: Serialize,
    {
        bincode::serialize_into(buffer as &mut Vec<u8>, message)
            .map_err(ChannelError::serialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?; // Serialize `message` into `buffer`

        let nonce = self.0.nonce(); // Generate a new `nonce`

        self.0.hasher.reset(); // Compute the keyed hash..
        self.0.hasher.update(&nonce); // .. of `nonce`..
        self.0.hasher.update(&buffer); // .. and `buffer`..

        let tag = self.0.hasher.finalize(); // .. to obtain `tag`

        buffer.extend_from_slice(tag.as_bytes()); // Append `tag` to `buffer`

        Ok(())
    }
}

impl Receiver {
    pub fn decrypt<M>(
        &mut self,
        ciphertext: &[u8],
    ) -> Result<M, Top<ChannelError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        let nonce = self.0.nonce(); // Generate a new `nonce`

        let message = self
            .0
            .cipher
            .decrypt(&ChaChaNonce::from_slice(&nonce), ciphertext)
            .map_err(|_| ChannelError::DecryptFailed.into_top())
            .spot(here!())?; // Decrypt `ciphertext` to obtain `message`

        bincode::deserialize(&message)
            .map_err(ChannelError::deserialize_failed)
            .map_err(Doom::into_top)
            .spot(here!()) // Deserialize `message`
    }

    pub fn decrypt_in_place<M>(
        &mut self,
        ciphertext: &mut Vec<u8>,
    ) -> Result<M, Top<ChannelError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        let nonce = self.0.nonce(); // Generate a new `nonce`

        self.0
            .cipher
            .decrypt_in_place(
                &ChaChaNonce::from_slice(&nonce),
                &[],
                ciphertext as &mut Vec<u8>,
            )
            .map_err(|_| ChannelError::DecryptFailed.into_top())
            .spot(here!())?; // Decrypt `ciphertext` in place

        let plaintext = ciphertext; // `ciphertext` is now `plaintext`
        bincode::deserialize(plaintext)
            .map_err(ChannelError::deserialize_failed)
            .map_err(Doom::into_top)
            .spot(here!()) // Deserialize `plaintext`
    }

    pub fn authenticate<M>(
        &mut self,
        ciphertext: &[u8],
    ) -> Result<M, Top<ChannelError>>
    where
        M: for<'de> Deserialize<'de>,
    {
        let nonce = self.0.nonce(); // Generate a new `nonce`

        if ciphertext.len() < HASH_LENGTH {
            // If `ciphertext` is shorter than `HASH_LENGTH`..
            ChannelError::AuthenticateFailed.fail().spot(here!()) // .. then it cannot contain an authentication tag
        } else {
            let (message, tag) =
                ciphertext.split_at(ciphertext.len() - HASH_LENGTH); // Split `ciphertext` into `message` and `tag` (`tag` is always second and `HASH_LENGTH` long)

            let tag: [u8; HASH_LENGTH] = tag.try_into().unwrap(); // This is guaranteed to work because `message.len() >= HASH_LENGTH`
            let tag: Hash = tag.into(); // Wrap `tag` into a `Hash`

            self.0.hasher.reset(); // Compute the keyed hash..
            self.0.hasher.update(&nonce); // of `nonce`..
            self.0.hasher.update(message); // .. and `buffer`..

            let digest = self.0.hasher.finalize(); // .. to obtain `digest`

            // IMPORTANT: The following equality MUST be computed between `Hash`es to ensure constant-time comparison!
            if tag == digest {
                bincode::deserialize(message)
                    .map_err(ChannelError::deserialize_failed)
                    .map_err(Doom::into_top)
                    .spot(here!()) // If `tag` is correct, deserialize `message`..
            } else {
                ChannelError::AuthenticateFailed.fail().spot(here!()) // .. otherwise, authentication failed
            }
        }
    }
}

impl State {
    fn nonce(&mut self) -> [u8; NONCE_LENGTH] {
        let mut nonce: [u8; NONCE_LENGTH] = self.nonce.to_be_bytes()
            [16 - NONCE_LENGTH..]
            .try_into()
            .unwrap(); // Initialize `nonce` to the `NONCE_LENGTH` least significant bytes of `self.nonce` (in Little Endian representation)

        nonce[0] = self.lane as u8; // Set the first byte of `nonce` to the representation of `self.lane`
                                    // As a result of this, `nonce` can effectively span over 2^(11 * 8) ~ 3E26 messages

        self.nonce += 1; // Increment `self.nonce`

        nonce
    }
}

pub fn channel(key: SharedKey, role: Role) -> (Sender, Receiver) {
    let key = key.to_bytes();

    let cipher_key = ChaChaKey::from_slice(&key);
    let hasher_key = key;

    // Corresponding ends of opposite roles must match
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

    #[test]
    fn authenticate_correct() {
        let (
            (mut alice_sender, mut alice_receiver),
            (mut bob_sender, mut bob_receiver),
        ) = setup();

        for message in 0..128u32 {
            let ciphertext = alice_sender.authenticate(&message).unwrap();
            let plaintext: u32 =
                bob_receiver.authenticate(&ciphertext[..]).unwrap();

            assert_eq!(plaintext, message);

            let ciphertext = bob_sender.authenticate(&message).unwrap();
            let plaintext: u32 =
                alice_receiver.authenticate(&ciphertext[..]).unwrap();

            assert_eq!(plaintext, message);
        }
    }

    #[test]
    fn authenticate_compromise() {
        let ((mut alice_sender, _), (_, mut bob_receiver)) = setup();

        let mut ciphertext = alice_sender.authenticate(&33u32).unwrap();
        ciphertext[3] = ciphertext[3].wrapping_add(1);

        assert!(bob_receiver.authenticate::<u32>(&ciphertext[..]).is_err());
    }

    #[test]
    fn authenticate_compromise_then_correct() {
        let ((mut alice_sender, _), (_, mut bob_receiver)) = setup();

        let mut ciphertext = alice_sender.authenticate(&33u32).unwrap();
        ciphertext[3] = ciphertext[3].wrapping_add(1);

        let _ = bob_receiver.authenticate::<u32>(&ciphertext[..]);

        let ciphertext = alice_sender.authenticate(&34u32).unwrap();
        let plaintext: u32 =
            bob_receiver.authenticate(&ciphertext[..]).unwrap();

        assert_eq!(plaintext, 34u32);
    }
}
