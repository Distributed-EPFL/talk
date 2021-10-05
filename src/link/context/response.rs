use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::link::context) enum Response {
    ContextAccepted,
    ContextRefused,
}
