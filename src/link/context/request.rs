use crate::link::context::ContextId;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::link::context) enum Request {
    Context(ContextId),
}
