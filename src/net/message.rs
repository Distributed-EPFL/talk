use serde::{de::DeserializeOwned, Serialize};

pub trait Message: 'static + Send + Sync + Serialize + DeserializeOwned {}

impl<M> Message for M where M: 'static + Send + Sync + Serialize + DeserializeOwned {}
