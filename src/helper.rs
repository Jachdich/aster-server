use rand::prelude::*;
use tokio::sync::mpsc;
use crate::Shared;

pub fn gen_uuid() -> i64 {
    (random::<u64>() >> (64 - 53)) as i64 // generate 53 bit integer because javascript is fucking dumb
}

pub type LockedState<'a> = tokio::sync::MutexGuard<'a, Shared>;
pub type JsonValue = serde_json::Value;
pub const NO_UID: i64 = 0;
