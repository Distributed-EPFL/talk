use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Scope(ScopeId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
enum ScopeId {
    Talk = 0,
    User = 1,
}

impl Scope {
    pub const fn user() -> Self {
        Scope(ScopeId::User)
    }

    pub(crate) const fn talk() -> Self {
        Scope(ScopeId::Talk)
    }
}
