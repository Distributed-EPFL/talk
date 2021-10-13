use crate::{
    crypto::{primitives::sign::PublicKey, KeyCard},
    link::rendezvous::{Request, Response, ServerSettings, ShardId},
    net::PlainConnection,
    sync::fuse::{Fuse, Relay},
};

use doomstack::{here, Doom, ResultExt, Top};

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io;
use tokio::net::{TcpListener, ToSocketAddrs};

pub struct Server {
    fuse: Fuse,
}

#[derive(Doom)]
pub enum ServerError {
    #[doom(description("Failed to initialize server: {}", source))]
    #[doom(wrap(initialize_failed))]
    InitializeFailed { source: io::Error },
}

#[derive(Doom)]
enum ListenError {
    #[doom(description("`listen` interrupted"))]
    ListenInterrupted,
}

#[derive(Doom)]
enum ServeError {
    #[doom(description("`serve` interrupted"))]
    ServeInterrupted,
    #[doom(description("connection error"))]
    ConnectionError,
}

struct Database {
    shards: Vec<HashSet<PublicKey>>,
    cards: HashMap<PublicKey, KeyCard>,
    membership: HashMap<PublicKey, Option<ShardId>>,
    addresses: HashMap<PublicKey, SocketAddr>,
}

impl Server {
    pub async fn new<A>(
        address: A,
        settings: ServerSettings,
    ) -> Result<Self, Top<ServerError>>
    where
        A: ToSocketAddrs,
    {
        let database = Arc::new(Mutex::new(Database {
            shards: settings
                .shard_sizes
                .iter()
                .map(|size| HashSet::with_capacity(*size))
                .collect(),
            cards: HashMap::new(),
            membership: HashMap::new(),
            addresses: HashMap::new(),
        }));

        let fuse = Fuse::new();
        let relay = fuse.relay();

        let listener = TcpListener::bind(address)
            .await
            .map_err(ServerError::initialize_failed)
            .map_err(Doom::into_top)
            .spot(here!())?;

        tokio::spawn(async move {
            let _ = Server::listen(settings, database, listener, relay).await;
        });

        Ok(Server { fuse })
    }

    async fn listen(
        settings: ServerSettings,
        database: Arc<Mutex<Database>>,
        listener: TcpListener,
        mut relay: Relay,
    ) -> Result<(), Top<ListenError>> {
        let fuse = Fuse::new();

        loop {
            if let Ok((stream, address)) = relay
                .map(listener.accept())
                .await
                .pot(ListenError::ListenInterrupted, here!())?
            {
                let settings = settings.clone();
                let database = database.clone();

                let connection: PlainConnection = stream.into();
                let relay = fuse.relay();

                tokio::spawn(async move {
                    let _ = Server::serve(
                        settings, database, connection, address, relay,
                    )
                    .await;
                });
            }

            // TODO: Log error case
        }
    }

    async fn serve(
        settings: ServerSettings,
        database: Arc<Mutex<Database>>,
        mut connection: PlainConnection,
        mut address: SocketAddr,
        mut relay: Relay,
    ) -> Result<(), Top<ServeError>> {
        let request: Request = relay
            .map(connection.receive())
            .await
            .pot(ServeError::ServeInterrupted, here!())?
            .pot(ServeError::ConnectionError, here!())?;

        let response = {
            let mut database = database.lock().unwrap();

            match request {
                Request::PublishCard(card, shard)
                    if database.cards.contains_key(&card.root()) =>
                {
                    let membership = database.membership[&card.root()];

                    if membership == shard {
                        Response::AcknowledgeCard
                    } else {
                        Response::AlreadyPublished(membership)
                    }
                }
                Request::PublishCard(_, Some(shard))
                    if (shard as usize) >= database.shards.len() =>
                {
                    Response::ShardIdInvalid
                }
                Request::PublishCard(_, Some(shard))
                    if database.shards[shard as usize].len()
                        >= settings.shard_sizes[shard as usize] =>
                {
                    Response::ShardFull
                }
                Request::PublishCard(card, shard) => {
                    if let Some(shard) = shard {
                        database.shards[shard as usize].insert(card.root());
                    }

                    database.membership.insert(card.root(), shard);
                    database.cards.insert(card.root(), card);

                    Response::AcknowledgeCard
                }

                Request::AdvertisePort(root, port) => {
                    address.set_port(port);
                    database.addresses.insert(root, address);

                    Response::AcknowledgePort
                }

                Request::GetShard(shard)
                    if (shard as usize) >= database.shards.len() =>
                {
                    Response::ShardIdInvalid
                }
                Request::GetShard(shard)
                    if database.shards[shard as usize].len()
                        < settings.shard_sizes[shard as usize] =>
                {
                    Response::ShardIncomplete
                }
                Request::GetShard(shard) => {
                    let shard = database.shards[shard as usize]
                        .iter()
                        .map(|key| database.cards[key].clone())
                        .collect::<Vec<_>>();

                    Response::Shard(shard)
                }

                Request::GetCard(root) => {
                    if let Some(card) = database.cards.get(&root) {
                        Response::Card(card.clone())
                    } else {
                        Response::CardUnknown
                    }
                }

                Request::GetAddress(root) => {
                    if let Some(address) = database.addresses.get(&root) {
                        Response::Address(address.clone())
                    } else {
                        Response::AddressUnknown
                    }
                }
            }
        };

        connection
            .send(&response)
            .await
            .pot(ServeError::ConnectionError, here!())?;

        Ok(())
    }
}
