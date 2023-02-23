use crate::schema::messages;
use crate::helper::JsonValue;
use serde::{Deserialize, Serialize};
use diesel::{Queryable, Insertable};

#[derive(Queryable, Clone, Debug, Serialize, Deserialize)]
pub struct CookedMessage {
    pub uuid: i64,
    pub content: String,
    pub author_uuid: i64,
    pub channel_uuid: i64,
    pub date: i32,
    #[serde(skip)]
    pub rowid: i64,
}

#[derive(Insertable, Clone)]
#[diesel(table_name = messages)]
pub struct CookedMessageInsertable {
    pub uuid: i64,
    pub content: String,
    pub author_uuid: i64,
    pub channel_uuid: i64,
    pub date: i32,
}

#[derive(Clone, Debug)]
pub enum MessageType {
    Raw(JsonValue),
    Cooked(CookedMessage),
    Internal(JsonValue),
}

pub enum Message {
    Broadcast(String),
    Received(MessageType),
}

impl CookedMessageInsertable {
    pub fn new(msg: CookedMessage) -> Self {
        return Self {
            uuid: msg.uuid,
            content: msg.content,
            author_uuid: msg.author_uuid,
            channel_uuid: msg.channel_uuid,
            date: msg.date,
        };
    }
}
