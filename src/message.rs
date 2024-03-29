use crate::schema::messages;
use diesel::{Insertable, Queryable};
use serde::{Deserialize, Serialize};

#[derive(Queryable, Clone, Debug, Serialize, Deserialize)]
#[diesel(table_name = messages)]
pub struct Message {
    pub uuid: i64,
    pub content: String,
    pub author_uuid: i64,
    pub channel_uuid: i64,
    pub date: i32,
    #[serde(skip)]
    pub rowid: i32,
}

#[derive(Insertable, Clone, Debug, Serialize, Deserialize)]
#[diesel(table_name = messages)]
pub struct NewMessage {
    pub uuid: i64,
    pub content: String,
    pub author_uuid: i64,
    pub channel_uuid: i64,
    pub date: i32,
}
