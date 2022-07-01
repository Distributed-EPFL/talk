use tokio_udt::UdtConnection;
use crate::net::Socket;

impl Socket for UdtConnection {}
