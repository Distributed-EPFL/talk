use crate::{
    crypto::{Identity, KeyCard},
    link::rendezvous::ShardId,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::link::rendezvous) enum Request {
    PublishCard(KeyCard, Option<ShardId>),
    AdvertisePort(Identity, u16),

    GetShard(ShardId),
    GetCard(Identity),
    GetAddress(Identity),
}
