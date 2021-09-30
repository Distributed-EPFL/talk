use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum FuseError {
    #[snafu(display("fuse burned"))]
    FuseBurned,
}
