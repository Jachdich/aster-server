use crate::schema::channels;
use crate::schema::users;
use crate::schema::groups;
use crate::schema::user_groups;
use crate::schema::sync_data;
use crate::schema::sync_servers;
use rand::prelude::*;

#[derive(Queryable, Insertable, Clone)]
#[table_name="channels"]
pub struct Channel {
    pub uuid: i64,
    pub name: String,
}

//message.rs for message models

#[derive(Queryable, Insertable, Clone)]
#[table_name="users"]
pub struct User {
    pub uuid: i64,
    pub name: String,
    pub pfp: String,
    pub group_uuid: i64,
}

#[derive(Queryable, Insertable, Clone)]
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

#[derive(Insertable, Clone)]
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

fn gen_uuid() -> i64 {
    (random::<u64>() >> 1) as i64
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
    pub fn as_json(&self) -> json::JsonValue {
        json::object!{name: self.name.clone().unwrap_or("".into()), uuid: self.server_uuid, ip: self.ip.clone(), port: self.port, pfp: self.pfp.clone().unwrap_or("".into())}
    }

    pub fn from_json(value: &json::JsonValue, user_uuid: i64, index: i32) -> Self {
        SyncServer {
            user_uuid: user_uuid,
            server_uuid: value["uuid"].as_i64().unwrap(),
            ip: value["ip"].as_str().unwrap().to_string(),
            port: value["port"].as_i32().unwrap(),
            pfp:  Some(value["pfp"].as_str().unwrap().to_string()),
            name: Some(value["name"].as_str().unwrap().to_string()),
            idx: index,
        }
    }
}

impl User {
    pub fn as_json(&self) -> json::JsonValue {
        json::object!{name: self.name.clone(), uuid: self.uuid, pfp: self.pfp.clone(), group_uuid: self.group_uuid}
    }
    pub fn from_json(value: &json::JsonValue) -> Self {
        User {
            name: value["name"].as_str().unwrap().to_string(),
            pfp: value["pfp"].as_str().unwrap().to_string(),
            uuid: value["uuid"].as_i64().unwrap(),
            group_uuid: value["group_uuid"].as_i64().unwrap(),
        }
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

    pub fn as_json(&self) -> json::JsonValue {
        json::object!{name: self.name.clone(), uuid: self.uuid}
    }
}

impl Group {
    fn as_json(&self) -> json::JsonValue {
        json::object!{name: self.name.clone(), perms: self.permissions, uuid: self.uuid, colour: self.colour}
    }
    fn from_json(value: &json::JsonValue) -> Self {
        Group {
            uuid: value["uuid"].as_i64().unwrap(),
            name: value["name"].to_string(),
            permissions: value["perms"].as_i64().unwrap(),
            colour: value["colour"].as_i32().unwrap(),
        }
    }
}
