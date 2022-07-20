use crate::net::Socket;
use tokio_udt::UdtConnection;

impl Socket for UdtConnection {}
