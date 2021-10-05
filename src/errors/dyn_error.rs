use std::error::Error;

pub type DynError = Box<dyn Error + 'static + Send + Sync>;
