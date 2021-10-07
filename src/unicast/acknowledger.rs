use crate::unicast::{Acknowledgement, Response};

use tokio::sync::mpsc::Sender;

type ResponseInlet = Sender<Response>;

pub struct Acknowledger {
    sequence: u32,
    acknowledgement: Acknowledgement,
    response_inlet: ResponseInlet,
}

impl Acknowledger {
    pub(in crate::unicast) fn new(
        sequence: u32,
        response_inlet: ResponseInlet,
    ) -> Self {
        Acknowledger {
            sequence,
            acknowledgement: Acknowledgement::Weak,
            response_inlet,
        }
    }

    pub fn weak(self) {
        // `self.acknowledgement` is already initialized to `Weak`
    }

    pub fn strong(mut self) {
        self.acknowledgement = Acknowledgement::Strong;
    }
}

impl Drop for Acknowledger {
    fn drop(&mut self) {
        let _ = self
            .response_inlet
            .try_send(Response::new(self.sequence, self.acknowledgement));
    }
}
