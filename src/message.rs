use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub uuid: i64,
    pub content: String,
    pub author_uuid: i64,
    pub channel_uuid: i64,
    pub date: i32,
    pub edited: bool,
    pub reply: Option<i64>,
}
