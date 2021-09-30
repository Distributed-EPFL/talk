use crate::crypto::Scope;

pub trait Statement {
    const SCOPE: Scope = Scope::user();

    type Header;
    const HEADER: Self::Header;
}
