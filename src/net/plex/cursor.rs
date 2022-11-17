use crate::net::plex::Role;
use std::sync::atomic::{AtomicU32, Ordering};

pub(in crate::net::plex) struct Cursor {
    role: Role,
    cursor: AtomicU32,
}

impl Cursor {
    pub fn new(role: Role) -> Self {
        match role {
            Role::Connector => Cursor {
                role: Role::Connector,
                cursor: AtomicU32::new(0),
            },
            Role::Listener => Cursor {
                role: Role::Listener,
                cursor: AtomicU32::new(u32::MAX),
            },
        }
    }

    pub fn next(&self) -> u32 {
        match self.role {
            Role::Connector => self.cursor.fetch_add(1, Ordering::Relaxed),
            Role::Listener => self.cursor.fetch_sub(1, Ordering::Relaxed),
        }
    }
}
