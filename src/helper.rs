use rand::prelude::*;
use tokio::sync::mpsc;
use crate::Shared;

pub fn gen_uuid() -> i64 {
    (random::<u64>() >> 1) as i64
}

pub type LockedState<'a> = tokio::sync::MutexGuard<'a, Shared>;
pub type JsonValue = serde_json::Value;
pub const NO_UID: i64 = 0;
