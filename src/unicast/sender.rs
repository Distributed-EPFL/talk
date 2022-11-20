use crate::{
    crypto::Identity,
    net::{Connector, Message as NetMessage},
    sync::fuse::{Fuse, Relay},
    unicast::{
        Acknowledgement, Caster, CasterError, CasterSettings, CasterTerminated, PushSettings,
        Request, SenderSettings,
    },
};
use doomstack::{here, Doom, ResultExt, Top};
use parking_lot::Mutex;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
    time::Instant,
};
use tokio::{sync::oneshot::Receiver, task::JoinHandle, time};

type AcknowledgementOutlet = Receiver<Result<Acknowledgement, Top<CasterError>>>;

#[derive(Clone)]
pub struct Sender<Message: NetMessage> {
    connector: Arc<dyn Connector>,
    database: Arc<Mutex<Database<Message>>>,
    settings: SenderSettings,
    _fuse: Arc<Fuse>,
}

struct Database<Message: NetMessage> {
    links: HashMap<Identity, Link<Message>>,
}

struct Link<Message: NetMessage> {
    caster: Caster<Message>,
    last_message: Instant,
}

#[derive(Doom)]
pub enum SenderError {
    #[doom(description("Failed to `send` message"))]
    SendFailed,
}

impl<Message> Sender<Message>
where
    Message: NetMessage,
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

            fuse.spawn(async move {
                let _ = Sender::keep_alive(database, settings).await;
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
        remote: Identity,
        message: Message,
    ) -> Result<Acknowledgement, Top<SenderError>> {
        self.post(remote, Request::Message(message))
            .await
            .unwrap()
            .pot(SenderError::SendFailed, here!())
    }

    pub fn run_send(
        &self,
        remote: Identity,
        message: Message,
        relay: Relay,
    ) -> JoinHandle<Option<Result<Acknowledgement, Top<SenderError>>>> {
        let post = self.post(remote, Request::Message(message));

        relay.run(async move { post.await.unwrap().pot(SenderError::SendFailed, here!()) })
    }

    pub fn spawn_send(
        &self,
        remote: Identity,
        message: Message,
        fuse: &Fuse,
    ) -> JoinHandle<Option<Result<Acknowledgement, Top<SenderError>>>> {
        self.run_send(remote, message, fuse.relay())
    }

    pub async fn push(&self, remote: Identity, message: Message, settings: PushSettings)
    where
        Message: Clone,
    {
        self.drive(remote, message, None, settings).await;
    }

    pub fn run_push(
        &self,
        remote: Identity,
        message: Message,
        settings: PushSettings,
        relay: Relay,
    ) -> JoinHandle<Option<()>>
    where
        Message: Clone,
    {
        let sender = self.clone();
        relay.run(async move { sender.push(remote, message, settings).await })
    }

    pub fn spawn_push(
        &self,
        remote: Identity,
        message: Message,
        settings: PushSettings,
        fuse: &Fuse,
    ) -> JoinHandle<Option<()>>
    where
        Message: Clone,
    {
        self.run_push(remote, message, settings, fuse.relay())
    }

    pub async fn push_brief(
        &self,
        remote: Identity,
        brief: Message,
        expanded: Message,
        settings: PushSettings,
    ) where
        Message: Clone,
    {
        self.drive(remote, brief, Some(expanded), settings).await;
    }

    pub fn run_push_brief(
        &self,
        remote: Identity,
        brief: Message,
        expanded: Message,
        settings: PushSettings,
        relay: Relay,
    ) -> JoinHandle<Option<()>>
    where
        Message: Clone,
    {
        let sender = self.clone();
        relay.run(async move { sender.push_brief(remote, brief, expanded, settings).await })
    }

    pub fn spawn_push_brief(
        &self,
        remote: Identity,
        brief: Message,
        expanded: Message,
        settings: PushSettings,
        fuse: &Fuse,
    ) -> JoinHandle<Option<()>>
    where
        Message: Clone,
    {
        self.run_push_brief(remote, brief, expanded, settings, fuse.relay())
    }

    pub async fn drive(
        &self,
        remote: Identity,
        mut message: Message,
        mut fallback: Option<Message>,
        settings: PushSettings,
    ) where
        Message: Clone,
    {
        let mut sleep_agent = settings.retry_schedule.agent();

        loop {
            if let Ok(acknowledgement) = self.send(remote, message.clone()).await {
                if acknowledgement >= settings.stop_condition {
                    break;
                }

                if acknowledgement == Acknowledgement::Expand {
                    if let Some(fallback) = fallback.take() {
                        message = fallback;
                        continue;
                    }
                }
            }

            sleep_agent.step().await;
        }
    }

    fn post(&self, remote: Identity, mut request: Request<Message>) -> AcknowledgementOutlet {
        let mut database = self.database.lock();

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

            request = match link.caster.post(request) {
                Ok(outlet) => break outlet,
                Err(CasterTerminated(request)) => {
                    database.links.remove(&remote);
                    request
                }
            };
        }
    }

    async fn keep_alive(database: Arc<Mutex<Database<Message>>>, settings: SenderSettings) {
        loop {
            database.lock().links.retain(|_, link| {
                (link.last_message.elapsed() <= settings.link_timeout)
                    && link.caster.post(Request::KeepAlive).is_ok()
            });

            time::sleep(settings.keepalive_interval).await;
        }
    }
}
