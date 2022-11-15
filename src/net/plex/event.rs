use crate::net::plex::Security;
use tokio::sync::mpsc::Sender as MpscSender;

type MessageInlet = MpscSender<(Security, Vec<u8>)>;

pub(in crate::net::plex) enum Event {
    NewPlex {
        plex: u32,
        message_inlet: MessageInlet,
    },
    Message {
        plex: u32,
        security: Security,
        message: Vec<u8>,
    },
    DropPlex {
        plex: u32,
    },
}
