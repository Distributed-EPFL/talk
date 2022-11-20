use crate::{
    crypto::Identity,
    link::context::{ContextId, ListenDispatcherSettings, Listener, Request, Response},
    net::{Listener as NetListener, SecureConnection},
    sync::fuse::Fuse,
};
use doomstack::{here, Doom, ResultExt, Top};
use parking_lot::Mutex;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};
use tokio::sync::mpsc::{self, Sender};

type Inlet = Sender<(Identity, SecureConnection)>;

pub struct ListenDispatcher {
    database: Arc<Mutex<Database>>,
    settings: ListenDispatcherSettings,
    fuse: Arc<Fuse>,
}

pub(in crate::link::context) struct Database {
    pub inlets: HashMap<ContextId, Inlet>,
}

#[derive(Doom)]
enum ServeError {
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

            fuse.spawn(async move {
                let _ = ListenDispatcher::listen(listener, database).await;
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

        let mut database = self.database.lock();

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

    async fn listen<L>(mut listener: L, database: Arc<Mutex<Database>>)
    where
        L: NetListener,
    {
        let fuse = Fuse::new();

        loop {
            if let Ok((remote, connection)) = listener.accept().await {
                let database = database.clone();

                fuse.spawn(async move {
                    let _ = ListenDispatcher::serve(remote, connection, database).await;
                });
            }
        }
    }

    async fn serve(
        remote: Identity,
        mut connection: SecureConnection,
        database: Arc<Mutex<Database>>,
    ) -> Result<(), Top<ServeError>> {
        let Request::Context(context) = connection
            .receive()
            .await
            .pot(ServeError::ReceiveFailed, here!())?;

        let inlet = database.lock().inlets.get(&context).map(Clone::clone);

        match inlet {
            Some(inlet) => {
                connection
                    .send(&Response::ContextAccepted)
                    .await
                    .pot(ServeError::SendFailed, here!())?;

                // This can only fail if the (local) receiving end is
                // dropped, in which case we don't care about the error
                let _ = inlet.try_send((remote, connection));

                Ok(())
            }
            None => {
                connection
                    .send(&Response::ContextRefused)
                    .await
                    .pot(ServeError::SendFailed, here!())?;

                ServeError::MissingContext { context }.fail().spot(here!())
            }
        }
    }
}
