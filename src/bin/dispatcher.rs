use std::convert::TryInto;
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime};
use tokio::time;

use talk::net::{
    DatagramDispatcher, DatagramDispatcherMode, DatagramDispatcherSettings, DatagramSender,
};

type Message = Vec<u8>;

const MESSAGES: usize = 20_000_000;

fn get_latency(ts: u128) -> Duration {
    let now = (SystemTime::now().duration_since(SystemTime::UNIX_EPOCH))
        .unwrap()
        .as_millis();
    Duration::from_millis((now as i128 - ts as i128) as u64)
}

async fn spam(sender: DatagramSender<Message>, destination: SocketAddr) {
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

    sender.pace(messages_iter, 250000.).await;

    time::sleep(Duration::from_secs(2)).await;

    dbg!(&sender.statistics);
}

async fn run() {
    let mut slots = vec![false; MESSAGES];

    let mut received = 0;
    let mut total = 0;

    let mode = match std::env::var("BROKER_ADDR") {
        Ok(broker_addr) if broker_addr != "" => DatagramDispatcherMode::Client {
            broker_addr: broker_addr.parse().unwrap(),
            sockets: 1,
        },
        _ => {
            let port: u16 = std::env::var("PORT").unwrap().parse().unwrap();
            DatagramDispatcherMode::Broker {
                bind_addr: format!("0.0.0.0:{port}").parse().unwrap(),
                inbound_sockets: 1,
                outbound_sockets: 1,
            }
        }
    };

    let dispatcher: DatagramDispatcher<Message, Message> = DatagramDispatcher::bind(
        &mode,
        DatagramDispatcherSettings {
            maximum_packet_rate: 450000.,
            ..Default::default()
        },
    )
    .unwrap();

    let (sender, mut receiver) = dispatcher.split();

    let sending = match mode {
        DatagramDispatcherMode::Client { broker_addr, .. } => {
            tokio::spawn(async move { spam(sender, broker_addr).await })
        }
        _ => tokio::spawn(async {}),
    };

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

    sending.await.unwrap();

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

fn main() {
    // core_affinity::set_for_current(core_affinity::CoreId { id: 0 });
    // let core_id = std::sync::Mutex::new(3);

    tokio::runtime::Builder::new_multi_thread()
        // .worker_threads(4)
        .enable_all()
        // .on_thread_start(move || {
        //     let mut core_id = core_id.lock().unwrap();
        //     core_affinity::set_for_current(core_affinity::CoreId { id: *core_id });
        //     *core_id += 1;
        // })
        .build()
        .unwrap()
        .block_on(async { run().await })
}
