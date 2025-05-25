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
}

// #[derive(Clone)]
// pub struct UserGroupConnection {
//     link_id: i32,
//     pub user_uuid: i64,
//     pub group_uuid: i64,
// }

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

impl Perm {
    fn combine(self, other: Perm) -> Perm {
        use Perm::*;
        match (self, other) {
            (_, Allow) => Allow,
            (_, Deny) => Deny,
            (s, Default) => s,
        }
    }
}

impl Permissions {
    fn apply_over(&self, other: Permissions) -> Permissions {
        Permissions {
            modify_channels: self.modify_channels.combine(other.modify_channels),
            modify_icon_name: self.modify_icon_name.combine(other.modify_icon_name),
            modify_groups: self.modify_groups.combine(other.modify_groups),
            modify_user_groups: self.modify_user_groups.combine(other.modify_user_groups),
            ban_users: self.ban_users.combine(other.ban_users),
            send_messages: self.send_messages.combine(other.send_messages),
            read_messages: self.read_messages.combine(other.read_messages),
            manage_messages: self.manage_messages.combine(other.manage_messages),
            join_voice: self.join_voice.combine(other.join_voice),
        }
    }
}

impl User {
    pub fn resolve_server_permissions(&self, state: &LockedState) -> Result<Permissions, DbError> {
        let mut base = state.get_base_perms()?;
        for group in &self.groups {
            let group_perms = state
                .get_group(*group)?
                .ok_or(DbError::QueryReturnedNoRows)?
                .permissions;
            base = base.apply_over(group_perms);
        }
        Ok(base)
    }
    pub fn resolve_channel_permissions(
        &self,
        channel_in: &Channel,
        state: &LockedState,
    ) -> Result<Permissions, DbError> {
        let defaults = self.resolve_server_permissions(state)?;
        // defaults.apply_over()
        // Ok(defaults)
    }
}
