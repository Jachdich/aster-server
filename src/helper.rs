use crate::Shared;
use rand::prelude::*;

pub fn gen_uuid() -> i64 {
    (random::<u64>() >> 1) as i64
}

pub type LockedState<'a> = tokio::sync::MutexGuard<'a, Shared>;
pub type JsonValue = serde_json::Value;
