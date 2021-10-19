use doomstack::{here, Doom, ResultExt, Top};

use std::future::Future;
use std::time::Duration;

use tokio::time;

#[derive(Doom)]
#[doom(description("Operation timed out"))]
pub struct Timeout;

pub async fn timeout<F, T>(
    duration: Duration,
    future: F,
) -> Result<T, Top<Timeout>>
where
    F: Future<Output = T>,
{
    time::timeout(duration, future)
        .await
        .map_err(|_| Timeout.into_top())
        .spot(here!())
}

pub async fn optional_timeout<F, T>(
    duration: Option<Duration>,
    future: F,
) -> Result<T, Top<Timeout>>
where
    F: Future<Output = T>,
{
    if let Some(duration) = duration {
        timeout(duration, future).await
    } else {
        Ok(future.await)
    }
}
