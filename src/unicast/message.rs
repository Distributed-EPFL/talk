use serde::{Deserialize, Serialize};

pub trait Message:
    'static + Send + Serialize + for<'de> Deserialize<'de>
{
}

impl<M> Message for M where
    M: 'static + Send + Serialize + for<'de> Deserialize<'de>
{
}
