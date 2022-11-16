use crate::net::plex::Role;

pub(in crate::net::plex) struct Cursor {
    role: Role,
    cursor: u32,
}

impl Cursor {
    pub fn new(role: Role) -> Self {
        match role {
            Role::Connector => Cursor {
                role: Role::Connector,
                cursor: 0,
            },
            Role::Listener => Cursor {
                role: Role::Listener,
                cursor: u32::MAX,
            },
        }
    }

    pub fn next(&mut self) -> u32 {
        let next = self.cursor;

        match self.role {
            Role::Connector => {
                self.cursor += 1;
            }
            Role::Listener => {
                self.cursor -= 1;
            }
        }

        next
    }
}
