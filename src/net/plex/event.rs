use crate::net::plex::{Message, PlexHandle};
use tokio::sync::mpsc::Sender as MpscSender;

type MessageInlet = MpscSender<Message>;

pub(in crate::net::plex) enum Event {
    NewPlex { plex: u32, handle: PlexHandle },
    Message { plex: u32, message: Message },
    DropPlex { plex: u32 },
}
