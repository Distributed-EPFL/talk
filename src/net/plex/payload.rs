use crate::net::plex::Header;

use super::security::Security;

pub(in crate::net::plex) enum Payload {
    NewPlex {
        plex: u32,
    },
    Message {
        plex: u32,
        security: Security,
        message: Vec<u8>,
    },
    ClosePlex {
        plex: u32,
    },
}

impl Payload {
    pub fn header(&self) -> Header {
        match self {
            Payload::NewPlex { plex } => Header::NewPlex { plex: *plex },
            Payload::Message { plex, security, .. } => Header::Message {
                plex: *plex,
                security: *security,
            },
            Payload::ClosePlex { plex } => Header::ClosePlex { plex: *plex },
        }
    }
}
