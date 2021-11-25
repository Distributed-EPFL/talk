use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(in crate::net) enum SessionControl {
    Connect,
    KeepAlive,
}
