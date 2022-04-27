use super::connection::UdtConnection;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use udt::Epoll;

#[derive(Default)]
pub struct UdtKernel {
    pub port: u16,
    pub connection: Option<UdtConnection>,
}

impl UdtKernel {
    pub async fn connect(&self, addr: SocketAddr) -> UdtConnection {
        let connection = UdtConnection::init();
        connection.socket.connect(addr).unwrap();
        let mut epoll = Epoll::create().unwrap();
        epoll.add_usock(&connection.socket, None).unwrap();

        tokio::task::spawn_blocking(move || {
            // wait for connection;
            epoll.wait(-1, false).unwrap();
        })
        .await
        .unwrap();

        connection
    }

    fn init_server(&mut self) -> &UdtConnection {
        if let Some(ref conn) = self.connection {
            return conn;
        }
        let connection = UdtConnection::init();
        connection
            .socket
            .bind(SocketAddr::new("127.0.0.1".parse().unwrap(), self.port))
            .unwrap();
        connection.socket.listen(1000).unwrap();
        self.connection = Some(connection);
        self.connection.as_ref().unwrap()
    }

    pub fn accept(&mut self) -> Option<(SocketAddr, UdtConnection)> {
        let connection = self.init_server();
        connection
            .socket
            .accept()
            .ok()
            .map(|(socket, addr)| (addr, UdtConnection { socket }))
        // loop {
        //     if let Ok((socket, addr)) = connection.socket.accept() {
        //         println!("Accepted connection from {}", addr);
        //         return (addr, UdtConnection { socket })
        //     }
        //     println!("Waiting for connection...");
        //     tokio::time::sleep(Duration::from_millis(100)).await;
        // }
    }

    pub async fn send(&self, addr: SocketAddr, msg: &[u8]) -> tokio::io::Result<()> {
        let mut connection = self.connect(addr).await;
        connection.write_all(msg).await?;
        Ok(())
    }

    pub async fn recv(&self, addr: SocketAddr) -> tokio::io::Result<Vec<u8>> {
        let mut buf = vec![];
        let mut connection = self.connect(addr).await;
        connection.read_buf(&mut buf).await?;
        Ok(buf)
    }
}
