use crate::net::Socket;

use tokio::net::TcpStream;

impl Socket for TcpStream {}
