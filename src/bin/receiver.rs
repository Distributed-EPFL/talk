use std::convert::TryInto;
use std::time::{Duration, Instant, SystemTime};
use tokio::time;

use talk::net::{DatagramDispatcher, DatagramDispatcherSettings};

type Message = Vec<u8>;

const MESSAGES: usize = 20000000;

fn get_latency(ts: u128) -> Duration {
    let now = (SystemTime::now().duration_since(SystemTime::UNIX_EPOCH))
        .unwrap()
        .as_millis();
    Duration::from_millis((now as i128 - ts as i128) as u64)
}

#[tokio::main]
async fn main() {
    let mut slots = vec![false; MESSAGES];

    let mut received = 0;
    let mut total = 0;

    let mut dispatcher: DatagramDispatcher<Message, Message> = DatagramDispatcher::bind(
        "0.0.0.0:1234",
        DatagramDispatcherSettings {
            maximum_packet_rate: 300000.,
            ..Default::default()
        },
    )
    .unwrap();

    let mut last_print = Instant::now();
    let mut last_received = 0;
    let mut last_total = 0;

    let mut max_latency = Duration::ZERO;

    while received < MESSAGES {
        let (_, message) = dispatcher.receive().await;

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

            dbg!(&dispatcher.sender.statistics);

            last_print = Instant::now();
            last_received = received;
            last_total = total;
        }
    }

    time::sleep(Duration::from_secs(1)).await;

    dbg!(&dispatcher.sender.statistics);
    println!("Received {} / {}", received, total);
    println!("Packets received: {}", dispatcher.packets_received());
    println!(
        "Packet loss: {:.02}%",
        (1. - received as f64 / dispatcher.packets_received() as f64) * 100.,
    )
}
