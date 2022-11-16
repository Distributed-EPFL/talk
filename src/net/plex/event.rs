use crate::{net::plex::Message, sync::fuse::Fuse};
use tokio::sync::mpsc::Sender as MpscSender;

type MessageInlet = MpscSender<Message>;

pub(in crate::net::plex) enum Event {
    NewPlex {
        plex: u32,
        receive_inlet: MessageInlet,
        fuse: Fuse,
    },
    Message {
        plex: u32,
        message: Message,
    },
    DropPlex {
        plex: u32,
    },
}
