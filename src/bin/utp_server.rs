use clap::Parser;
use utp::{UtpSocket, UtpListener};
use std::net::SocketAddr;

const MESSAGE_SIZE: usize = 100_000;

#[derive(Parser)]
struct Args {
    #[clap(short, default_value_t = 9000)]
    port: u16
}

fn handle_client(mut socket: UtpSocket) {
    let mut buf = vec![0u8; MESSAGE_SIZE];
    loop {
        let (received, _addr) = socket.recv_from(&mut buf).expect(
            &format!("Failed to receive message from {}", socket.peer_addr().unwrap())
        );
    }
}

fn main() {
    let args = Args::parse();

    let listener = UtpListener::bind(SocketAddr::new("0.0.0.0".parse().unwrap(), args.port))
        .expect("error binding socket");
        
    println!("Listing on {}. Waiting for connections...", listener.local_addr().unwrap());
    for connection in listener.incoming() {
        // Spawn a new handler for each new connection
        if let Ok((socket, _src)) = connection {
            println!("New connection from {}", socket.peer_addr().unwrap());
            std::thread::spawn(move || handle_client(socket));
        }
    }

}
