use crate::{
    crypto::{primitives::sign::PublicKey, KeyCard},
    link::rendezvous::ShardId,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::link::rendezvous) enum Request {
    PublishCard(KeyCard, Option<ShardId>),
    AdvertisePort(PublicKey, u16),

    GetShard(ShardId),
    GetCard(PublicKey),
    GetAddress(PublicKey),
}
