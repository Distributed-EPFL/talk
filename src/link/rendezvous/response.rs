use crate::{crypto::KeyCard, link::rendezvous::ShardId};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::link::rendezvous) enum Response {
    AcknowledgeCard,
    AcknowledgePort,

    Shard(Vec<KeyCard>),
    Card(KeyCard),
    Address(SocketAddr),

    AlreadyPublished(Option<ShardId>),
    ShardFull,
    ShardIdInvalid,
    ShardIncomplete,
    CardUnknown,
    AddressUnknown,
}
