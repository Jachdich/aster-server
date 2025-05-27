use crate::{
    helper::{LockedState, Uuid},
    permissions::{Perm, PermableEntity, Permissions},
    shared::DbError,
};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Channel {
    pub uuid: i64,
    pub name: String,
    pub position: usize,
    pub permissions: HashMap<PermableEntity, Permissions>,
}

//message.rs for message models

// TODO should users have permissions?
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct User {
    pub uuid: i64,
    pub name: String,
    pub pfp: String,
    #[serde(skip)]
    pub password: String, // hashed, don't you worry
    pub groups: Vec<Uuid>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Group {
    pub uuid: i64,
    pub permissions: Permissions,
    pub name: String,
    pub colour: i32,
    pub position: usize,
}

#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct SyncData {
    #[serde(skip)]
    pub user_uuid: i64,
    pub uname: String,
    pub pfp: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
