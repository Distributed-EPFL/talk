use futures::stream::{FuturesUnordered, StreamExt};

use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time;

const TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn join<I, T>(handles: I) -> Result<(), &'static str>
where
    I: IntoIterator<Item = JoinHandle<T>>,
{
    if time::timeout(
        TIMEOUT,
        handles
            .into_iter()
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>(),
    )
    .await
    .is_err()
    {
        Err("`join` failed: unfinished tasks remaining")
    } else {
        Ok(())
    }
}
