use snafu::Snafu;

type BincodeError = Box<bincode::ErrorKind>;

pub use hash::HashError;

pub(crate) mod hash {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum HashError {
        #[snafu(display("failed to serialize data: {}", source))]
        SerializeFailed { source: BincodeError },
    }
}
