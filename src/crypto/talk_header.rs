use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i8)]
pub(crate) enum TalkHeader {
    KeyCardPublicKeys = 0,
    SecureConnectionIdentityChallenge = 1,
}
