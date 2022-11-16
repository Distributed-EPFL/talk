use crate::net::plex::{Message, PlexHandle};

pub(in crate::net::plex) enum Event {
    NewPlex { plex: u32, handle: PlexHandle },
    Message { plex: u32, message: Message },
    DropPlex { plex: u32 },
    Ping,
}
