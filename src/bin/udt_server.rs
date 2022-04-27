use std::time::Duration;
use talk::net::udt::UdtKernel;

use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() {
    let mut kernel = UdtKernel {
        port: 2222,
        ..Default::default()
    };
    let mut connections = vec![];
    loop {
        if let Some(x) = kernel.accept() {
            connections.push(x);
        }
        for (addr, connection) in connections.iter_mut() {
            connection
                .write_all(format!("hello {}", &addr).as_bytes())
                .await
                .unwrap();
        }
        println!("Got {} connection(s)", connections.len());
        println!();
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}
