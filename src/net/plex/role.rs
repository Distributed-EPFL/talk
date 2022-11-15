#[derive(Clone, Copy)]
pub(in crate::net::plex) enum Role {
    Connector,
    Listener,
}
