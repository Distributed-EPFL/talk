use crate::net::plex::{Header, Message};

pub(in crate::net::plex) enum Payload {
    NewPlex { plex: u32 },
    Message { plex: u32, message: Message },
    DropPlex { plex: u32 },
    Ping,
    Pong,
}

impl Payload {
    pub fn header(&self) -> Header {
        match self {
            Payload::NewPlex { plex } => Header::NewPlex { plex: *plex },
            Payload::Message { plex, message, .. } => Header::Message {
                plex: *plex,
                security: message.security,
            },
            Payload::DropPlex { plex } => Header::DropPlex { plex: *plex },
            Payload::Ping => Header::Ping,
            Payload::Pong => Header::Pong,
        }
    }
}
