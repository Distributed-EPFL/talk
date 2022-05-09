use clap::Parser;
use std::net::SocketAddr;
use libutp_rs::{UtpContext, UtpSocket};
use futures::StreamExt;
use tokio::io::AsyncReadExt;

const BUFFER_SIZE: usize = 10_000_000;

#[derive(Parser)]
struct Args {
    #[clap(short, default_value_t = 9000)]
    port: u16,

    #[clap(long)]
    mtu: Option<u16>
}

async fn handle_client(mut socket: UtpSocket) {
    let mut buf = Vec::with_capacity(BUFFER_SIZE);
    let mut total = 0;
    loop {
        let read = socket.read_buf(&mut buf).await.expect(
            &format!("Failed to receive message")
        );
        if read == 0 {
            break
        }
        total += read;
        if buf.len() > 100_000 {
            buf.truncate(0);
        }
    }
    println!("Bytes read: {}", total);
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let addr = SocketAddr::new("0.0.0.0".parse().unwrap(), args.port);

    let context = UtpContext::bind(addr)
        .expect("error binding socket");

    if let Some(mtu) = args.mtu {
        context.set_udp_mtu(mtu);
    }

    let mut listener = context.listener();
        
    println!("Listing on {}. Waiting for connections...", addr);
    loop {
        let socket = listener.next().await.unwrap().unwrap();
        // Spawn a new handler for each new connection
        println!("New connection from {}", socket.peer_addr());
        tokio::spawn(async move {
            handle_client(socket).await
        });
    }
}
