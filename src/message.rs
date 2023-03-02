use crate::schema::messages;
use serde::{Deserialize, Serialize};
use diesel::{Queryable, Insertable};

#[derive(Queryable, Insertable, Clone, Debug, Serialize, Deserialize)]
#[diesel(table_name = messages)]
pub struct Message {
    pub uuid: i64,
    pub content: String,
    pub author_uuid: i64,
    pub channel_uuid: i64,
    pub date: i32,
}
