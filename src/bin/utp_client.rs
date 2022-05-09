use clap::Parser;
use std::time::Instant;
use std::net::SocketAddr;

use libutp_rs::{UtpContext};
use tokio::io::AsyncWriteExt;


#[derive(Parser)]
struct Args {
    server_ip: String,

    #[clap(short, default_value_t = 9000)]
    port: u16,

    #[clap(long)]
    mtu: Option<u16>
}

const NB_MESSAGES: usize = 10_000;
const MESSAGE_SIZE: usize = 200_000;

#[tokio::main]
async fn main() {
    // Connect to an hypothetical local server running on port 8080
    let args = Args::parse();

    let remote_addr = SocketAddr::new(args.server_ip.parse().unwrap(), args.port);

    let context = UtpContext::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    if let Some(mtu) = args.mtu {
        context.set_udp_mtu(mtu);
    }
    let mut socket = context.connect(remote_addr).await.expect("Error connecting to remote peer");

    let start = Instant::now();

    let random_bytes: Vec<u8> = (0..MESSAGE_SIZE).map(|_| { rand::random() }).collect();

    for i in 0..NB_MESSAGES {
        if i % 1000 == 0 {
            println!("Sent {} messages", i);
        }
        // let mut sent = 0;
        socket.write_all(&random_bytes).await.expect("write failed");
    }

    // Close the stream
    // stream.close().expect("Error closing connection");

    let nb_bytes = NB_MESSAGES*MESSAGE_SIZE;
    let nb_ms =  start.elapsed().as_millis();

    println!("Sent {} bytes in {} secs", nb_bytes, nb_ms as f64 / 1e3);
    println!("Throughput: {} Mbps", nb_bytes as f64 * 8. / (nb_ms as f64 / 1e3) / 1e6);
}
