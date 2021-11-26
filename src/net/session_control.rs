use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::net) enum SessionControl {
    Connect,
    KeepAlive,
}
