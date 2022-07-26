use std::time::{Duration, SystemTime};

use talk::net::{DatagramDispatcher, DatagramDispatcherSettings};

use tokio::time;

const MESSAGES: u64 = 32000000;
const WORKERS: u64 = 4;

const MESSAGES_PER_WORKER: u64 = MESSAGES / WORKERS;

async fn spam(worker: u64) {
    let destination = std::env::var("SERVER")
        .unwrap_or("172.31.11.87:1234".to_owned())
        .parse()
        .unwrap();

    let dispatcher = DatagramDispatcher::bind(
        "0.0.0.0:0",
        DatagramDispatcherSettings {
            workers: 1,
            retransmission_delay: Duration::from_millis(200),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    for index in (worker * MESSAGES_PER_WORKER)..((worker + 1) * MESSAGES_PER_WORKER) {
        let message = index.to_be_bytes();
        let mut message = message.as_slice().to_vec();

        let ts = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        message.extend_from_slice(&ts.to_be_bytes());
        message.extend_from_slice([42u8; 1000].as_slice());

        dispatcher.send(destination, message).await;

        if index % 200 == 0 {
            time::sleep(Duration::from_millis(1)).await;
        }
    }

    loop {
        time::sleep(Duration::from_secs(1)).await;
    }
}

#[tokio::main]
async fn main() {
    for worker in 0..WORKERS {
        tokio::spawn(spam(worker));
    }

    loop {
        time::sleep(Duration::from_secs(1)).await;
    }
}
