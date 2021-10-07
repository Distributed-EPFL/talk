use crate::unicast::Acknowledgement;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::unicast) struct Response {
    sequence: u32,
    acknowledgement: Acknowledgement,
}

impl Response {
    pub fn new(sequence: u32, acknowledgement: Acknowledgement) -> Self {
        Response {
            sequence,
            acknowledgement,
        }
    }
}
