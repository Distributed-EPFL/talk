use serde::{Deserialize, Serialize};

pub trait Message:
    'static + Send + Serialize + for<'de> Deserialize<'de>
{
}
