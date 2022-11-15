use crate::net::plex::Security;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub(in crate::net::plex) enum Header {
    NewPlex { plex: u32 },
    Message { plex: u32, security: Security },
    ClosePlex { plex: u32 },
}
