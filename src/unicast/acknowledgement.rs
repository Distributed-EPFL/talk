use serde::{Deserialize, Serialize};
use std::cmp::{Ord, Ordering, PartialOrd};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Acknowledgement {
    Weak,
    Expand,
    Strong,
}

impl PartialOrd for Acknowledgement {
    fn partial_cmp(&self, rho: &Self) -> Option<Ordering> {
        Some(self.cmp(rho))
    }
}

impl Ord for Acknowledgement {
    fn cmp(&self, rho: &Self) -> Ordering {
        fn score(acknowledgement: &Acknowledgement) -> u8 {
            match acknowledgement {
                Acknowledgement::Weak => 0,
                Acknowledgement::Expand => 1,
                Acknowledgement::Strong => 2,
            }
        }

        score(self).cmp(&score(rho))
    }
}
