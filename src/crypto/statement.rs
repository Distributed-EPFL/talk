use crate::crypto::Scope;

use serde::Serialize;

pub trait Statement: Serialize {
    const SCOPE: Scope = Scope::user();

    type Header: Serialize;
    const HEADER: Self::Header;
}
