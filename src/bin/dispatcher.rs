use std::convert::TryInto;
use std::time::{Duration, Instant, SystemTime};
use tokio::time;

use talk::net::{DatagramDispatcher, DatagramDispatcherSettings, DatagramSender};

type Message = Vec<u8>;

const MESSAGES: usize = 20_000_000;

fn get_latency(ts: u128) -> Duration {
    let now = (SystemTime::now().duration_since(SystemTime::UNIX_EPOCH))
        .unwrap()
        .as_millis();
    Duration::from_millis((now as i128 - ts as i128) as u64)
}

async fn spam(sender: DatagramSender<Message>) {
    let destination = std::env::var("SERVER")
        .unwrap_or("127.0.0.1:1234".to_owned())
        .parse()
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

    sender.pace(messages_iter, 200000.).await;

    time::sleep(Duration::from_secs(2)).await;

    dbg!(&sender.statistics);
}

#[tokio::main]
async fn main() {
    let mut slots = vec![false; MESSAGES];

    let mut received = 0;
    let mut total = 0;

    let dispatcher: DatagramDispatcher<Message, Message> = DatagramDispatcher::bind(
        "0.0.0.0:1234",
        DatagramDispatcherSettings {
            maximum_packet_rate: 450000.,
            ..Default::default()
        },
    )
    .unwrap();

    let (sender, mut receiver) = dispatcher.split();

    tokio::spawn(async {
        spam(sender).await
    });

    let mut last_print = Instant::now();
    let mut last_received = 0;
    let mut last_total = 0;

    let mut max_latency = Duration::ZERO;

    while received < MESSAGES {
        let (_, message) = receiver.receive().await;

        let mut index = [0u8; 8];
        (&mut index[..]).clone_from_slice(&message.as_slice()[..8]);
        let index = u64::from_be_bytes(index) as usize;

        let ts = u128::from_be_bytes(message[8..24].try_into().unwrap());
        let latency = get_latency(ts);
        if latency > max_latency {
            max_latency = latency;
        }

        if !slots[index] {
            slots[index] = true;
            received += 1;
        }

        total += 1;

        if last_print.elapsed().as_secs_f64() >= 1. {
            let instant_received = received - last_received;
            let instant_total = total - last_total;

            println!(
                "Received {} / {} ({} / {} instant). Max latency: {:?}",
                received, total, instant_received, instant_total, max_latency,
            );

            dbg!(&receiver.statistics);

            last_print = Instant::now();
            last_received = received;
            last_total = total;
        }
    }

    time::sleep(Duration::from_secs(1)).await;

    dbg!(&receiver.statistics);
    println!("Received {} / {}", received, total);
    println!("Packets received: {}", receiver.packets_received());
    dbg!(max_latency);
    println!(
        "Packet loss: {:.02}%",
        (1. - received as f64 / receiver.packets_received() as f64) * 100.,
    )
}
