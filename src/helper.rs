use crate::Shared;
use rand::prelude::*;

pub fn gen_uuid() -> i64 {
    (random::<u64>() >> (64 - 53)) as i64 // generate 53 bit integer because javascript is fucking dumb
}

pub type LockedState<'a> = tokio::sync::MutexGuard<'a, Shared>;
pub type JsonValue = serde_json::Value;
pub type Uuid = i64;
