use std::sync::Arc;

use tokio::sync::watch::Sender as WatchInlet;

pub struct Poster<T> {
    inlet: WatchInlet<Option<Arc<T>>>,
}

impl<T> Poster<T> {
    pub(in crate::sync::board) fn from_inlet(inlet: WatchInlet<Option<Arc<T>>>) -> Self {
        Poster { inlet }
    }

    pub fn post(self, value: T) {
        self.inlet.send_replace(Some(Arc::new(value)));
    }
}
