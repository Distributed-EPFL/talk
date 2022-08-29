use std::time::{Duration, SystemTime};

use talk::net::{DatagramDispatcher, DatagramDispatcherSettings};

use tokio::time;

const MESSAGES: u64 = 20000000;

type Message = Vec<u8>;

async fn spam() {
    let destination = std::env::var("SERVER")
        .unwrap_or("127.0.0.1:1234".to_owned())
        .parse()
        .unwrap();

    let dispatcher: DatagramDispatcher<Message, Message> = DatagramDispatcher::bind(
        "0.0.0.0:0",
        DatagramDispatcherSettings {
            // retransmission_delay: Duration::from_millis(150),
            maximum_packet_rate: 300000.,
            ..Default::default()
        },
    )
    .unwrap();

    time::sleep(Duration::from_secs(2)).await;

    let messages_iter = (0..MESSAGES).map(|index| {
        let message = index.to_be_bytes();
        let mut message = message.as_slice().to_vec();

        let ts = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        message.extend_from_slice(&ts.to_be_bytes());
        message.extend_from_slice([42u8; 1000].as_slice());
        (destination, message)
    });

    dispatcher.sender.pace(messages_iter, 200000.).await;

    time::sleep(Duration::from_secs(2)).await;

    dbg!(&dispatcher.sender.statistics);
}

#[tokio::main]
async fn main() {
    spam().await;
}
