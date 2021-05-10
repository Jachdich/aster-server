use rand::prelude::*;
use tokio::sync::mpsc;
use crate::message::MessageType;

pub fn gen_uuid() -> i64 {
    (random::<u64>() >> 1) as i64
}

pub type Tx = mpsc::UnboundedSender<MessageType>;
