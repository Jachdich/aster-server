use crate::schema::*;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use diesel::{Queryable, Insertable};

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

#[derive(Queryable, Insertable, Clone)]
#[diesel(table_name=sync_data)]
pub struct SyncData {
    pub user_uuid: i64,
    pub uname: String,
    pub pfp: String,
}

#[derive(Insertable, Clone, Serialize, Deserialize)]
#[diesel(table_name=sync_servers)]
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
#[diesel(table_name=emojis)]
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

impl Channel {
    pub fn new(name: &str) -> Self {
        let uuid: i64 = gen_uuid();
        return Channel {
            uuid,
            name: name.to_string(),
        };
    }
}
