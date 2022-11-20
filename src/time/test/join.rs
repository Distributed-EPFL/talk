use doomstack::{Doom, Top};
use futures::stream::{FuturesUnordered, StreamExt};
use std::time::Duration;
use tokio::{task::JoinHandle, time};

const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Doom)]
#[doom(description("Failed to `join`: unfinished tasks remaining"))]
pub struct JoinError;

pub async fn join<I>(handles: I) -> Result<(), Top<JoinError>>
where
    I: IntoIterator<Item = JoinHandle<()>>,
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
