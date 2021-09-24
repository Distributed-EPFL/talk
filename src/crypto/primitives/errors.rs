use snafu::Snafu;

type BincodeError = Box<bincode::ErrorKind>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum HasherError {
    #[snafu(display("failed to serialize data: {}", source))]
    SerializeError { source: BincodeError },
}
