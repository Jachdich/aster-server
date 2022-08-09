use crate::schema::channels;
use crate::schema::users;
use crate::schema::groups;
use crate::schema::user_groups;
use crate::schema::sync_data;
use crate::schema::sync_servers;
use crate::schema::emojis;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use crate::helper::JsonValue;

#[derive(Queryable, Insertable, Clone, Serialize)]
#[table_name="channels"]
pub struct Channel {
    pub uuid: i64,
    pub name: String,
}

//message.rs for message models

#[derive(Queryable, Insertable, Clone, Serialize, Deserialize)]
#[table_name="users"]
pub struct User {
    pub uuid: i64,
    pub name: String,
    pub pfp: String,
    pub group_uuid: i64,
}

#[derive(Queryable, Insertable, Clone, Serialize, Deserialize)]
#[table_name="groups"]
pub struct Group {
    pub uuid: i64,
    pub permissions: i64,
    pub name: String,
    pub colour: i32,
}

#[derive(Queryable, Insertable, Clone)]
#[table_name="user_groups"]
pub struct UserGroupConnection {
    link_id: i32,
    pub user_uuid: i64,
    pub group_uuid: i64,
}

#[derive(Queryable, Insertable, Clone)]
#[table_name="sync_data"]
pub struct SyncData {
    pub user_uuid: i64,
    pub uname: String,
    pub pfp: String,
}

#[derive(Insertable, Clone, Serialize, Deserialize)]
#[table_name="sync_servers"]
pub struct SyncServer {
    pub user_uuid: i64,
    pub server_uuid: i64,
    pub ip: String,
    pub port: i32,
    pub pfp:  Option<String>,
    pub name: Option<String>,
    pub idx: i32,
}

#[derive(Queryable, Clone)]
pub struct SyncServerQuery {
    pub user_uuid: i64,
    pub server_uuid: i64,
    pub ip: String,
    pub port: i32,
    pub pfp:  Option<String>,
    pub name: Option<String>,
    pub idx: i32,
    pub rowid: i32,
}

#[derive(Queryable, Insertable, Clone, Serialize)]
#[table_name="emojis"]
pub struct Emoji {
    pub uuid: i64,
    pub name: String,
    pub data: String,
}

fn gen_uuid() -> i64 {
    (random::<u64>() >> 1) as i64
}

impl Emoji {
    pub fn new(uuid: i64, name: String, data: String) -> Self {
        Self {
            uuid, name, data
        }
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
            server_uuid: item.server_uuid,
            ip: item.ip,
            port: item.port,
            pfp: item.pfp,
            name: item.name,
            idx: item.idx,
        }
    }
}

impl SyncServer {
    pub fn from_json(value: JsonValue, user_uuid: i64, index: i32) -> Result<Self, serde_json::Error> {
        let mut s: Self = serde_json::from_value(value)?;
        s.idx = index;
        s.user_uuid = user_uuid;
        Ok(s)
    }
}

impl Channel {
    pub fn new(name: &str) -> Self {
        let uuid: i64 = gen_uuid();
        return Channel {
            uuid: uuid,
            name: name.to_string(),
        };
    }
}
