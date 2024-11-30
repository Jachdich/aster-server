use crate::helper::gen_uuid;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
pub struct Channel {
    pub uuid: i64,
    pub name: String,
}

//message.rs for message models

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub uuid: i64,
    pub name: String,
    pub pfp: String,
    pub group_uuid: i64,
    pub password: String, // hashed, don't you worry
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Group {
    pub uuid: i64,
    pub permissions: i64,
    pub name: String,
    pub colour: i32,
}

#[derive(Clone)]
pub struct UserGroupConnection {
    link_id: i32,
    pub user_uuid: i64,
    pub group_uuid: i64,
}

#[derive(Clone, Serialize)]
pub struct SyncData {
    #[serde(skip)]
    pub user_uuid: i64,
    pub uname: String,
    pub pfp: String,
}

#[derive(Clone, Serialize, Deserialize)]
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

#[derive(Clone, Serialize)]
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

impl Channel {
    pub fn new(name: &str) -> Self {
        let uuid = gen_uuid();
        Channel {
            uuid,
            name: name.to_string(),
        }
    }
}
