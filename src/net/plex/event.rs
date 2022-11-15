use crate::net::plex::Security;
use tokio::sync::mpsc::Sender as MpscSender;

type BytesInlet = MpscSender<Vec<u8>>;

pub(in crate::net::plex) enum Event {
    NewPlex {
        plex: u32,
        message_inlet: BytesInlet,
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
