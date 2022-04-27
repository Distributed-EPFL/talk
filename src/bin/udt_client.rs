use talk::net::udt::UdtKernel;

use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() {
    let kernel = UdtKernel {
        port: 2223,
        ..Default::default()
    };
    let mut connection = kernel.connect("127.0.0.1:2222".parse().unwrap()).await;
    loop {
        let mut buf = vec![];
        connection.read_buf(&mut buf).await.unwrap();
        println!("Got message: {} bytes", buf.len());
        println!("{}", std::str::from_utf8(&buf).unwrap());
        println!();
    }
}
