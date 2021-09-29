use chacha20poly1305::aead::{
    Aead as ChaChaAead, AeadInPlace as ChaChaAeadInPlace,
};
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaChaNonce};

use crate::crypto::primitives::{
    errors::{
        channel::{DeserializeFailed, SerializeFailed},
        ChannelError,
    },
    exchange::Role,
};

use serde::{Deserialize, Serialize};

use snafu::ResultExt;

use std::convert::TryInto;

const NONCE_LENGTH: usize = 12;

pub struct Sender(State);
pub struct Receiver(State);

struct State {
    cipher: ChaCha20Poly1305,
    role: Role,
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
        buffer.clear();

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

        nonce[0] = self.role as u8;
        self.nonce += 1;

        *ChaChaNonce::from_slice(&nonce)
    }
}
