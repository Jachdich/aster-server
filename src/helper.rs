use rand::prelude::*;
use tokio::sync::mpsc;
use crate::message::MessageType;
use crate::Shared;

pub fn gen_uuid() -> i64 {
    (random::<u64>() >> 1) as i64
}

pub type Tx = mpsc::UnboundedSender<MessageType>;
pub type LockedState<'a> = tokio::sync::MutexGuard<'a, Shared>;
pub const NO_UID: i64 = 0;
