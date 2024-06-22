use crate::helper::gen_uuid;
use crate::schema::*;
use diesel::{Insertable, Queryable};
use serde::{Deserialize, Serialize};

#[derive(Queryable, Insertable, Clone, Serialize)]
#[diesel(table_name=channels)]
pub struct Channel {
    pub uuid: i64,
    pub name: String,
}

//message.rs for message models

#[derive(Queryable, Insertable, Clone, Serialize, Deserialize)]
#[diesel(table_name=users)]
pub struct User {
    pub uuid: i64,
    pub name: String,
    pub pfp: String,
    pub group_uuid: i64,
}

#[derive(Queryable, Insertable, Clone, Serialize, Deserialize)]
#[diesel(table_name=groups)]
pub struct Group {
    pub uuid: i64,
    pub permissions: i64,
    pub name: String,
    pub colour: i32,
}

#[derive(Queryable, Insertable, Clone)]
#[diesel(table_name=user_groups)]
pub struct UserGroupConnection {
    link_id: i32,
    pub user_uuid: i64,
    pub group_uuid: i64,
}

#[derive(Queryable, Insertable, Clone, Serialize)]
#[diesel(table_name=sync_data)]
pub struct SyncData {
    #[serde(skip)]
    pub user_uuid: i64,
    pub uname: String,
    pub pfp: String,
}

#[derive(Insertable, Clone, Serialize, Deserialize)]
#[diesel(table_name=sync_servers)]
pub struct SyncServer {
    #[serde(skip)]
    pub user_uuid: i64,
    pub uuid: Option<i64>,
    pub uname: String,
    pub ip: String,
    pub port: i32,
    pub pfp: Option<String>,
    pub name: Option<String>,
    pub idx: i32,
}

#[derive(Queryable, Clone)]
pub struct SyncServerQuery {
    pub user_uuid: i64,
    pub uuid: Option<i64>,
    pub uname: String,
    pub ip: String,
    pub port: i32,
    pub pfp: Option<String>,
    pub name: Option<String>,
    pub idx: i32,
    pub rowid: i32,
}

#[derive(Queryable, Insertable, Clone, Serialize)]
#[diesel(table_name=emojis)]
pub struct Emoji {
    pub uuid: i64,
    pub name: String,
    pub data: String,
}

impl Emoji {
    pub fn new(uuid: i64, name: String, data: String) -> Self {
        Self { uuid, name, data }
    }
}

impl SyncData {
    pub fn new(uuid: i64) -> Self {
        Self {
            user_uuid: uuid,
            pfp: "".into(),
            uname: "".into(),
        }
    }
}

impl From<SyncServerQuery> for SyncServer {
    fn from(item: SyncServerQuery) -> Self {
        SyncServer {
            user_uuid: item.user_uuid,
            uuid: item.uuid,
            ip: item.ip,
            port: item.port,
            pfp: item.pfp,
            name: item.name,
            idx: item.idx,
            uname: item.uname,
        }
    }
}

impl Channel {
    pub fn new(name: &str) -> Self {
        let uuid = gen_uuid();
        Channel {
            uuid,
            name: name.to_string(),
        }
    }
}
