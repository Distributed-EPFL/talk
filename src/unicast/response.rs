use crate::unicast::Acknowledgement;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::unicast) enum Response {
    Acknowledgement(u32, Acknowledgement),
}
