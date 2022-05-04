use clap::Parser;
use utp::UtpStream;
use std::io::Write;
use std::time::Instant;

#[derive(Parser)]
struct Args {
    server_ip: String,

    #[clap(short, default_value_t = 9000)]
    port: u16
}

const NB_MESSAGES: usize = 10_000;
const MESSAGE_SIZE: usize = 100_000;


fn main() {
    // Connect to an hypothetical local server running on port 8080
    let args = Args::parse();

    let addr = format!("{}:{}", args.server_ip, args.port);
    let mut stream = UtpStream::connect(addr).expect("Error connecting to remote peer");

    let start = Instant::now();

    let random_bytes: Vec<u8> = (0..MESSAGE_SIZE).map(|_| { rand::random() }).collect();

    for i in 0..NB_MESSAGES {
        let mut sent = 0;
        while sent < MESSAGE_SIZE {
            sent += stream.write(&random_bytes[sent..]).expect("Write failed");
        }
        println!("Sent {} messages", i);
    }

    // Close the stream
    stream.close().expect("Error closing connection");

    let nb_bytes = NB_MESSAGES*MESSAGE_SIZE;
    let nb_secs =  start.elapsed().as_secs();

    println!("Sent {} bytes in {} secs", nb_bytes, nb_secs);
    println!("Throughput: {} Mbps", nb_bytes as f64 * 8. / nb_secs as f64 / 1e6);
}
