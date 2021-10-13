use crate::{
    crypto::primitives::sign::PublicKey,
    link::context::{
        ContextId, ListenDispatcherSettings, Listener, Request, Response,
    },
    net::{Listener as NetListener, SecureConnection},
    sync::fuse::{Fuse, Relay},
};

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use doomstack::{here, Doom, ResultExt, Top};

use tokio::sync::mpsc::{self, Sender};

type Inlet = Sender<(PublicKey, SecureConnection)>;

pub struct ListenDispatcher {
    database: Arc<Mutex<Database>>,
    settings: ListenDispatcherSettings,
    fuse: Arc<Fuse>,
}

pub(in crate::link::context) struct Database {
    pub inlets: HashMap<ContextId, Arc<Mutex<Inlet>>>,
}

#[derive(Doom)]
enum ListenError {
    #[doom(description("`listen` interrupted"))]
    ListenInterrupted,
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

    pub fn register(&mut self, context: ContextId) -> Listener {
        let (inlet, outlet) = mpsc::channel(self.settings.channel_capacity);

        match self.database.lock().unwrap().inlets.entry(context.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(Mutex::new(inlet)));
            }
            Entry::Occupied(_) => {
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
        loop {
            if let Ok((remote, mut connection)) = relay
                .map(listener.accept())
                .await
                .pot(ListenError::ListenInterrupted, here!())?
            {
                if let Ok(Request::Context(context)) =
                    connection.receive().await
                {
                    let inlet = database
                        .lock()
                        .unwrap()
                        .inlets
                        .get(&context)
                        .map(Clone::clone);

                    if let Some(inlet) = inlet {
                        if connection
                            .send(&Response::ContextAccepted)
                            .await
                            .is_ok()
                        {
                            let _ = inlet
                                .lock()
                                .unwrap()
                                .try_send((remote, connection));
                        }
                    }
                } else {
                    let _ = connection.send(&Response::ContextRefused).await;
                }
            }
        }
    }
}
