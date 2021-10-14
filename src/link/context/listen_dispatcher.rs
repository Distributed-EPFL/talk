use crate::{
    crypto::primitives::sign::PublicKey,
    link::context::{
        ContextId, ListenDispatcherSettings, Listener, Request, Response,
    },
    net::{Listener as NetListener, SecureConnection},
    sync::fuse::{Fuse, Relay},
};

use doomstack::{here, Doom, ResultExt, Top};

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{self, Sender};

type Inlet = Sender<(PublicKey, SecureConnection)>;

pub struct ListenDispatcher {
    database: Arc<Mutex<Database>>,
    settings: ListenDispatcherSettings,
    fuse: Arc<Fuse>,
}

pub(in crate::link::context) struct Database {
    pub inlets: HashMap<ContextId, Inlet>,
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
    #[doom(description("Failed to receive context"))]
    ReceiveFailed,
    #[doom(description("Failed to send context acknowledgement"))]
    SendFailed,
    #[doom(description("`ContextId` missing: \"{}\"", context))]
    MissingContext { context: ContextId },
}

impl ListenDispatcher {
    pub fn new<L>(listener: L, settings: ListenDispatcherSettings) -> Self
    where
        L: NetListener,
    {
        let database = Arc::new(Mutex::new(Database {
            inlets: HashMap::new(),
        }));

        let fuse = Arc::new(Fuse::new());

        {
            let database = database.clone();
            let relay = fuse.relay();

            tokio::spawn(async move {
                let _ =
                    ListenDispatcher::listen(listener, database, relay).await;
            });
        }

        ListenDispatcher {
            database,
            settings,
            fuse,
        }
    }

    pub fn register(&self, context: ContextId) -> Listener {
        let (inlet, outlet) = mpsc::channel(self.settings.channel_capacity);

        let mut database = self.database.lock().unwrap();

        match database.inlets.entry(context.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(inlet);
            }
            Entry::Occupied(_) => {
                drop(database);
                panic!("called `register` twice for the same `context`")
            }
        }

        Listener::new(context, outlet, self.database.clone(), self.fuse.clone())
    }

    async fn listen<L>(
        mut listener: L,
        database: Arc<Mutex<Database>>,
        mut relay: Relay,
    ) -> Result<(), Top<ListenError>>
    where
        L: NetListener,
    {
        let fuse = Fuse::new();

        loop {
            if let Ok((remote, connection)) = relay
                .map(listener.accept())
                .await
                .pot(ListenError::ListenInterrupted, here!())?
            {
                let relay = fuse.relay();
                let database = database.clone();

                tokio::spawn(async move {
                    let _ = ListenDispatcher::serve(
                        remote, connection, database, relay,
                    )
                    .await;
                });
            }
        }
    }

    async fn serve(
        remote: PublicKey,
        mut connection: SecureConnection,
        database: Arc<Mutex<Database>>,
        mut relay: Relay,
    ) -> Result<(), Top<ServeError>> {
        let Request::Context(context) = relay
            .map(connection.receive())
            .await
            .pot(ServeError::ServeInterrupted, here!())?
            .pot(ServeError::ReceiveFailed, here!())?;

        let inlet = database
            .lock()
            .unwrap()
            .inlets
            .get(&context)
            .map(Clone::clone);

        match inlet {
            Some(inlet) => {
                relay
                    .map(connection.send(&Response::ContextAccepted))
                    .await
                    .pot(ServeError::ServeInterrupted, here!())?
                    .pot(ServeError::SendFailed, here!())?;

                // This can only fail if the (local) receiving end is
                // dropped, in which case we don't care about the error
                let _ = inlet.try_send((remote, connection));

                Ok(())
            }
            None => {
                relay
                    .map(connection.send(&Response::ContextRefused))
                    .await
                    .pot(ServeError::ServeInterrupted, here!())?
                    .pot(ServeError::SendFailed, here!())?;

                ServeError::MissingContext { context }.fail().spot(here!())
            }
        }
    }
}
