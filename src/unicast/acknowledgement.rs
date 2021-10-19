use serde::{Deserialize, Serialize};

use std::cmp::{Ord, Ordering, PartialOrd};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Acknowledgement {
    Weak,
    Strong,
}

impl PartialOrd for Acknowledgement {
    fn partial_cmp(&self, rho: &Self) -> Option<Ordering> {
        Some(self.cmp(rho))
    }
}

impl Ord for Acknowledgement {
    fn cmp(&self, rho: &Self) -> Ordering {
        match (self, rho) {
            (Acknowledgement::Weak, Acknowledgement::Strong) => Ordering::Less,
            (Acknowledgement::Strong, Acknowledgement::Weak) => {
                Ordering::Greater
            }
            _ => Ordering::Equal,
        }
    }
}
