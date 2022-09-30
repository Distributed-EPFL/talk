use crate::sync::board::Poster;

use std::sync::Arc;

use tokio::sync::watch::{self, Receiver as WatchOutlet};

pub struct Board<T> {
    state: State<T>,
}

enum State<T> {
    Posted(Arc<T>),
    Dropped,
    Pending(WatchOutlet<Option<Arc<T>>>),
}

impl<T> Board<T> {
    pub fn posted(value: T) -> Self {
        Board {
            state: State::Posted(Arc::new(value)),
        }
    }

    pub fn blank() -> (Board<T>, Poster<T>) {
        let (inlet, outlet) = watch::channel(None);

        let board = Board {
            state: State::Pending(outlet),
        };

        let poster = Poster::from_inlet(inlet);

        (board, poster)
    }

    pub async fn as_ref(&mut self) -> Option<&T> {
        let update = if let State::Pending(outlet) = &mut self.state {
            let _ = outlet.changed().await;

            if let Some(value) = outlet.borrow_and_update().as_ref() {
                Some(State::Posted(value.clone()))
            } else {
                Some(State::Dropped)
            }
        } else {
            None
        };

        if let Some(update) = update {
            self.state = update;
        }

        match &self.state {
            State::Posted(value) => Some(value.as_ref()),
            State::Dropped => None,
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    use tokio::time;

    #[tokio::test]
    async fn posted() {
        let mut board = Board::posted(33u32);
        assert_eq!(board.as_ref().await.unwrap(), &33);
    }

    #[tokio::test]
    async fn pending_post_ref() {
        let (mut board, poster) = Board::blank();
        poster.post(33u32);
        assert_eq!(board.as_ref().await.unwrap(), &33);
    }

    #[tokio::test]
    async fn pending_ref_post() {
        let (mut board, poster) = Board::blank();

        tokio::spawn(async move {
            time::sleep(Duration::from_millis(200)).await;
            poster.post(33u32);
        });

        assert_eq!(board.as_ref().await.unwrap(), &33);
    }

    #[tokio::test]
    async fn drop() {
        let (mut board, _) = Board::<u32>::blank();
        assert_eq!(board.as_ref().await, None);
    }
}
