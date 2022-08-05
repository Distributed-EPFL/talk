use crate::{
    crypto::{Identity, KeyCard},
    link::rendezvous::listener::RawListener,
    link::rendezvous::{Request, Response, ServerSettings, ShardId},
    net::{traits::TransportProtocol, PlainConnection},
    sync::fuse::Fuse,
};

use doomstack::{here, Doom, ResultExt, Top};

use parking_lot::Mutex;

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
};

use tokio::{io, net::TcpListener};
use tokio_udt::UdtListener;

pub struct Server {
    _fuse: Fuse,
}

#[derive(Doom)]
pub enum ServerError {
    #[doom(description("Failed to initialize server: {}", source))]
    #[doom(wrap(initialize_failed))]
    InitializeFailed { source: io::Error },
}

#[derive(Doom)]
enum ServeError {
    #[doom(description("connection error"))]
    ConnectionError,
}

struct Database {
    shards: Vec<HashSet<Identity>>,
    cards: HashMap<Identity, KeyCard>,
    membership: HashMap<Identity, Option<ShardId>>,
    addresses: HashMap<Identity, SocketAddr>,
}

impl Server {
    pub async fn new(
        address: SocketAddr,
        settings: ServerSettings,
    ) -> Result<Self, Top<ServerError>> {
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

        let listener = {
            let result = match settings.connect.transport {
                TransportProtocol::TCP => TcpListener::bind(address).await.map(RawListener::Tcp),
                TransportProtocol::UDT(ref config) => {
                    UdtListener::bind(address, Some(config.clone()))
                        .await
                        .map(RawListener::Udt)
                }
            };
            result
                .map_err(ServerError::initialize_failed)
                .map_err(Doom::into_top)
                .spot(here!())?
        };

        fuse.spawn(async move {
            let _ = Server::listen(settings, database, listener).await;
        });

        Ok(Server { _fuse: fuse })
    }

    async fn listen(
        settings: ServerSettings,
        database: Arc<Mutex<Database>>,
        listener: RawListener,
    ) {
        let fuse = Fuse::new();

        let accept = || async {
            match listener {
                RawListener::Tcp(ref tcp_listener) => tcp_listener
                    .accept()
                    .await
                    .map(|(stream, address)| (stream.into(), address)),
                RawListener::Udt(ref udt_listener) => udt_listener
                    .accept()
                    .await
                    .map(|(address, stream)| (stream.into(), address)),
            }
        };

        loop {
            if let Ok((connection, address)) = accept().await {
                let connection: PlainConnection = connection;
                let settings = settings.clone();
                let database = database.clone();

                fuse.spawn(async move {
                    let _ = Server::serve(settings, database, connection, address).await;
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
    ) -> Result<(), Top<ServeError>> {
        let request: Request = connection
            .receive()
            .await
            .pot(ServeError::ConnectionError, here!())?;

        let response = {
            let mut database = database.lock();

            match request {
                Request::PublishCard(card, shard)
                    if database.cards.contains_key(&card.identity()) =>
                {
                    let membership = database.membership[&card.identity()];

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
                        database.shards[shard as usize].insert(card.identity());
                    }

                    database.membership.insert(card.identity(), shard);
                    database.cards.insert(card.identity(), card);

                    Response::AcknowledgeCard
                }

                Request::AdvertisePort(identity, port) => {
                    address.set_port(port);
                    database.addresses.insert(identity, address);

                    Response::AcknowledgePort
                }

                Request::GetShard(shard) if (shard as usize) >= database.shards.len() => {
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

                Request::GetCard(identity) => {
                    if let Some(card) = database.cards.get(&identity) {
                        Response::Card(card.clone())
                    } else {
                        Response::CardUnknown
                    }
                }

                Request::GetAddress(identity) => {
                    if let Some(address) = database.addresses.get(&identity) {
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
