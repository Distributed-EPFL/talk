use crate::sync::fuse::Relay;
use std::future::Future;
use tokio::{
    sync::{broadcast, broadcast::Sender},
    task::JoinHandle,
};

pub struct Fuse {
    sender: Sender<()>,
}

impl Fuse {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1);
        Fuse { sender }
    }

    pub fn relay(&self) -> Relay {
        Relay::new(self.sender.subscribe())
    }

    pub fn spawn<F>(&self, future: F) -> JoinHandle<Option<F::Output>>
    where
        F: 'static + Send + Future,
        F::Output: 'static + Send,
    {
        self.relay().run(future)
    }

    pub fn burn(self) {
        // This function is empty on purpose: `Drop::drop(self)` is called here
    }
}

impl Drop for Fuse {
    fn drop(&mut self) {
        let _ = self.sender.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn uninterrupted_single() {
        let fuse = Fuse::new();
        let mut relay = fuse.relay();

        let (s, r) = oneshot::channel();

        tokio::spawn(async move {
            s.send(0).unwrap();
        });

        assert_eq!(relay.map(r).await.unwrap().unwrap(), 0);
    }

    #[tokio::test]
    async fn uninterrupted_multiple() {
        let fuse = Fuse::new();
        let mut relay = fuse.relay();

        let (s0, r0) = oneshot::channel();
        let (s1, r1) = oneshot::channel();
        let (s2, r2) = oneshot::channel();
        let (s3, r3) = oneshot::channel();

        tokio::spawn(async move {
            s0.send(0).unwrap();
            s1.send(1).unwrap();
            s2.send(2).unwrap();
            s3.send(3).unwrap();
        });

        assert_eq!(relay.map(r0).await.unwrap().unwrap(), 0);
        assert_eq!(relay.map(r1).await.unwrap().unwrap(), 1);
        assert_eq!(relay.map(r2).await.unwrap().unwrap(), 2);
        assert_eq!(relay.map(r3).await.unwrap().unwrap(), 3);
    }

    #[tokio::test]
    async fn interrupted_single() {
        let fuse = Fuse::new();
        let mut relay = fuse.relay();

        let (s, r) = oneshot::channel();

        tokio::spawn(async move {
            s.send(0).unwrap();
        });

        fuse.burn();

        assert!(relay.map(r).await.is_none());
    }

    #[tokio::test]
    async fn interrupted_multiple() {
        let fuse = Fuse::new();
        let mut relay = fuse.relay();

        let (s0, r0) = oneshot::channel();
        let (s1, r1) = oneshot::channel();
        let (s2, r2) = oneshot::channel();
        let (s3, r3) = oneshot::channel();

        tokio::spawn(async move {
            s0.send(0).unwrap();
            s1.send(1).unwrap();
            s2.send(2).unwrap();
            s3.send(3).unwrap();
        });

        assert_eq!(relay.map(r0).await.unwrap().unwrap(), 0);
        assert_eq!(relay.map(r1).await.unwrap().unwrap(), 1);

        fuse.burn();

        assert!(relay.map(r2).await.is_none());
        assert!(relay.map(r3).await.is_none());
    }

    #[tokio::test]
    async fn interrupted_on_spawn() {
        let fuse = Fuse::new();
        let mut relay = fuse.relay();

        let (s0, r0) = oneshot::channel();
        let (s1, r1) = oneshot::channel();
        let (s2, r2) = oneshot::channel();
        let (s3, r3) = oneshot::channel();

        tokio::spawn(async move {
            s0.send(0).unwrap();
            s1.send(1).unwrap();
            s2.send(2).unwrap();
            s3.send(3).unwrap();

            fuse.burn();
        })
        .await
        .unwrap();

        // Fuse is already burned here

        assert!(relay.map(r0).await.is_none());
        assert!(relay.map(r1).await.is_none());
        assert!(relay.map(r2).await.is_none());
        assert!(relay.map(r3).await.is_none());
    }
}
