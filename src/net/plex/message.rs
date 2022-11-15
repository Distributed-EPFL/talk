use crate::net::plex::Security;

pub(in crate::net::plex) struct Message {
    pub security: Security,
    pub message: Vec<u8>,
}
