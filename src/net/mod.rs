mod errors;
mod socket;

pub(crate) mod plain_connection;
pub(crate) mod secure_connection;

pub mod sockets;

pub use plain_connection::PlainConnection;
pub use socket::Socket;
