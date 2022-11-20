use crate::{
    link::context::{Connector, ContextId},
    net::Connector as NetConnector,
};
use parking_lot::Mutex;
use std::{collections::HashSet, sync::Arc};

pub struct ConnectDispatcher {
    connector: Arc<dyn NetConnector>,
    database: Arc<Mutex<Database>>,
}

pub(in crate::link::context) struct Database {
    pub contexts: HashSet<ContextId>,
}

impl ConnectDispatcher {
    pub fn new<C>(connector: C) -> Self
    where
        C: NetConnector,
    {
        let connector = Arc::new(connector);

        let database = Arc::new(Mutex::new(Database {
            contexts: HashSet::new(),
        }));

        ConnectDispatcher {
            connector,
            database,
        }
    }

    pub fn register(&self, context: ContextId) -> Connector {
        if self.database.lock().contexts.insert(context.clone()) {
            Connector::new(context, self.connector.clone(), self.database.clone())
        } else {
            panic!("called `register` twice for the same `context`");
        }
    }
}
