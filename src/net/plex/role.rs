#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::net::plex) enum Role {
    Connector,
    Listener,
}
