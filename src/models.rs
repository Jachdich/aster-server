use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug)]
enum Perm {
    Allow,
    Deny,
    Default,
}

#[derive(Clone, PartialEq, Debug)]
struct ServerPerms {
    manage_channels: Perm,
    change_icon_name: Perm,
    channel_perms: ChannelPerms,
}

#[derive(Clone, PartialEq, Debug)]
struct ChannelPerms {
    send_messages: Perm,
    read_messages: Perm,
    manage_messages: Perm,
}

#[derive(Clone, PartialEq, Debug)]
enum PermableEntity {
    User(User),
    Group(Group),
}

#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Channel {
    pub uuid: i64,
    pub name: String,
    pub position: usize,
    #[serde(skip)]
    pub perms: Vec<(PermableEntity, ChannelPerms)>,
}

//message.rs for message models

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct User {
    pub uuid: i64,
    pub name: String,
    pub pfp: String,
    pub group_uuid: i64,
    #[serde(skip)]
    pub password: String, // hashed, don't you worry
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
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
