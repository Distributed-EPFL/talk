use std::convert::TryInto;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use tokio::time;

use atomic_counter::{AtomicCounter, RelaxedCounter};

use talk::net::{DatagramDispatcher, DatagramDispatcherSettings, DatagramSender};

type Message = Vec<u8>;

const MESSAGES: usize = 4_000_000;

fn get_latency(ts: u128) -> Duration {
    let now = (SystemTime::now().duration_since(SystemTime::UNIX_EPOCH))
        .unwrap()
        .as_millis();
    Duration::from_millis((now as i128 - ts as i128) as u64)
}

async fn spam(sender: DatagramSender<Message>) {
    let destination = match std::env::var("SERVER") {
        Ok(dest) => dest.parse().unwrap(),
        Err(_) => loop {
            time::sleep(Duration::from_secs(2)).await;
        },
    };

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

    sender.pace(messages_iter, 120_000.).await;

    time::sleep(Duration::from_secs(2)).await;

    dbg!(&sender.statistics);
}

async fn run() {
    let total = Arc::new(RelaxedCounter::new(0));

    let port: u16 = std::env::var("PORT").unwrap_or("0".into()).parse().unwrap();

    let dispatcher: DatagramDispatcher<Message, Message> = DatagramDispatcher::bind(
        ("0.0.0.0", port),
        DatagramDispatcherSettings {
            maximum_packet_rate: 350000.,
            ..Default::default()
        },
    )
    .unwrap();

    let (sender, mut receiver) = dispatcher.split();

    tokio::spawn(async { spam(sender).await });

    let mut last_total = 0;

    let max_latency = Arc::new(RwLock::new(Duration::ZERO));

    tokio::spawn({
        let max_latency = max_latency.clone();
        let total = total.clone();
        let statistics = receiver.statistics.clone();

        async move {
            loop {
                time::sleep(Duration::from_secs(1)).await;
                let instant_total = total.get() - last_total;

                println!(
                    "Received{} ({} instant). Max latency: {:?}",
                    total.get(),
                    instant_total,
                    max_latency.read().unwrap(),
                );

                dbg!(&statistics);

                last_total = total.get();
            }
        }
    });

    loop {
        let (_, message) = receiver.receive().await;

        let mut index = [0u8; 8];
        (&mut index[..]).clone_from_slice(&message.as_slice()[..8]);
        let _index = u64::from_be_bytes(index) as usize;

        let ts = u128::from_be_bytes(message[8..24].try_into().unwrap());
        let latency = get_latency(ts);
        {
            let mut max_latency = max_latency.write().unwrap();
            if latency > *max_latency {
                *max_latency = latency;
            }
        }

        total.inc();
    }

    // sending.await.unwrap();

    // time::sleep(Duration::from_secs(1)).await;

    // dbg!(&receiver.statistics);
    // println!("Received {} / {}", received, total);
    // println!("Packets received: {}", receiver.packets_received());
    // dbg!(max_latency);
    // println!(
    //     "Packet loss: {:.02}%",
    //     (1. - received as f64 / receiver.packets_received() as f64) * 100.,
    // )
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
