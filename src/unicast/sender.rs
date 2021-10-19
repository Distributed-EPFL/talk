use crate::{
    crypto::primitives::sign::PublicKey,
    net::Connector,
    sync::fuse::{Fuse, Relay},
    unicast::{
        Acknowledgement, Caster, CasterError, CasterSettings, CasterTerminated,
        Message as UnicastMessage, Request, SenderSettings,
    },
};

use doomstack::{here, Doom, ResultExt, Top};

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::oneshot::Receiver;
use tokio::time;

type AcknowledgementOutlet =
    Receiver<Result<Acknowledgement, Top<CasterError>>>;

pub struct Sender<Message: UnicastMessage> {
    connector: Arc<dyn Connector>,
    database: Arc<Mutex<Database<Message>>>,
    settings: SenderSettings,
    _fuse: Arc<Fuse>,
}

struct Database<Message: UnicastMessage> {
    links: HashMap<PublicKey, Link<Message>>,
}

struct Link<Message: UnicastMessage> {
    caster: Caster<Message>,
    last_message: Instant,
}

#[derive(Doom)]
pub enum SenderError {
    #[doom(description("Failed to `send` message"))]
    SendFailed,
}

#[derive(Doom)]
enum KeepAliveError {
    #[doom(description("`keep_alive` interrupted"))]
    KeepAliveInterrupted,
}

impl<Message> Sender<Message>
where
    Message: UnicastMessage,
{
    pub fn new<C>(connector: C, settings: SenderSettings) -> Self
    where
        C: Connector,
    {
        let connector = Arc::new(connector);

        let database = Arc::new(Mutex::new(Database {
            links: HashMap::new(),
        }));

        let fuse = Arc::new(Fuse::new());

        {
            let database = database.clone();
            let settings = settings.clone();
            let relay = fuse.relay();

            tokio::spawn(async move {
                let _ = Sender::keep_alive(database, relay, settings).await;
            });
        }

        Sender {
            connector,
            database,
            settings,
            _fuse: fuse,
        }
    }

    pub async fn send(
        &self,
        remote: PublicKey,
        message: Message,
    ) -> Result<Acknowledgement, Top<SenderError>> {
        self.push(remote, Request::Message(message))
            .await
            .unwrap()
            .pot(SenderError::SendFailed, here!())
    }

    fn push(
        &self,
        remote: PublicKey,
        mut request: Request<Message>,
    ) -> AcknowledgementOutlet {
        let mut database = self.database.lock().unwrap();

        loop {
            let link = match database.links.entry(remote) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => entry.insert(Link {
                    caster: Caster::new(
                        self.connector.clone(),
                        remote,
                        CasterSettings::from_sender_settings(&self.settings),
                    ),
                    last_message: Instant::now(),
                }),
            };

            request = match link.caster.push(request) {
                Ok(outlet) => break outlet,
                Err(CasterTerminated(request)) => {
                    database.links.remove(&remote);
                    request
                }
            };
        }
    }

    async fn keep_alive(
        database: Arc<Mutex<Database<Message>>>,
        mut relay: Relay,
        settings: SenderSettings,
    ) -> Result<(), Top<KeepAliveError>> {
        loop {
            database.lock().unwrap().links.retain(|_, link| {
                if link.last_message.elapsed() > settings.link_timeout {
                    false
                } else {
                    link.caster.push(Request::KeepAlive).is_ok()
                }
            });

            relay
                .map(time::sleep(settings.keepalive_interval))
                .await
                .pot(KeepAliveError::KeepAliveInterrupted, here!())?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{net::test::System, unicast::Receiver};

    #[tokio::test]
    async fn simple() {
        let System {
            keys,
            connectors,
            listeners,
        } = System::setup(2).await;

        let mut connectors = connectors.into_iter();
        let connector = connectors.next().unwrap();

        let mut listeners = listeners.into_iter().skip(1);
        let listener = listeners.next().unwrap();

        let mut keys = keys.into_iter().skip(1);
        let key = keys.next().unwrap();

        let sender: Sender<u32> = Sender::new(connector, Default::default());

        let mut receiver: Receiver<u32> =
            Receiver::new(listener, Default::default());

        tokio::spawn(async move {
            for reference in 0..32 {
                let (_, value, _) = receiver.receive().await;
                assert_eq!(value, reference);
            }
        });

        for value in 0..32 {
            sender.send(key, value).await.unwrap();
        }
    }
}
