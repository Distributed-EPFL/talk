use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::net::plex) enum Security {
    Secure,
    Plain,
    Raw,
}
