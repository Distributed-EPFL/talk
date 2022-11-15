use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub(in crate::net::plex) enum Security {
    Secure,
    Plain,
    Raw,
}
