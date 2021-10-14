use doomstack::{Doom, Top};

use futures::stream::{FuturesUnordered, StreamExt};

use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time;

const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Doom)]
#[doom(description("Failed to `join`: unfinished tasks remaining"))]
pub(crate) struct JoinError;

pub(crate) async fn join<I, T>(handles: I) -> Result<(), Top<JoinError>>
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
    .is_ok()
    {
        Ok(())
    } else {
        JoinError.fail()
    }
}
