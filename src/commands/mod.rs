pub mod auth;
mod log_any;
mod log_in;
mod log_out;

use log_any::*;
use log_in::*;
use log_out::*;

use crate::helper::{gen_uuid, JsonValue, LockedState, Uuid};
use crate::message::Message;
use crate::peer::Peer;

use crate::models::{Channel, Emoji, Group, SyncData, SyncServer, User};
use crate::permissions::{Perm, Permissions};
use crate::shared::DbError;
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::json;
use std::collections::HashMap;
use std::error::Error;

#[derive(Clone, Copy, Debug)]
pub enum Status {
    Ok = 200,
    BadRequest = 400,
    InternalError = 500,
    Unauthenticated = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    Conflict = 409,
}

impl Serialize for Status {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(*self as i32)
    }
}

#[derive(Deserialize)]
pub struct CreateGroupRequest {
    pub permissions: Permissions,
    pub name: String,
    pub colour: i32,
    pub position: usize,
}

#[derive(Deserialize)]
pub struct DeleteGroupRequest {
    pub uuid: Uuid,
}

#[derive(Deserialize)]
pub struct UpdateGroupRequest {
    pub uuid: i64,
    pub permissions: Option<Permissions>,
    pub name: Option<String>,
    pub colour: Option<i32>,
    pub position: Option<usize>,
}

#[derive(Deserialize)]
pub struct UpdateUserGroupsRequest {
    pub user: Uuid,
    pub groups: Vec<Uuid>,
}

#[derive(Deserialize)]
pub struct GetLastReadsRequest;

#[derive(Deserialize)]
pub struct MarkUnreadRequest {
    uuid: Uuid,
}

#[derive(Deserialize)]
pub struct GetNumUnreadRequest {
    channel: Uuid,
}

/// # API Docs
/// For documentation of each packet, its fields, and when it may be sent; see its respective struct's documentation.
/// ## Overview
/// The API uses json objects to communicate. Each packet consists of a json string on one line, followed by
/// a newline character indicating the end of the packet. The json string must contain a "command" property
/// which is the packet type. The server must respond to this packet with at least one packet with the same
/// value in its "command" field, and a "status" field containing one of a subset of HTTP status codes to
/// indicate whether the command succeeded or not. An example packet may look like this:
/// ```text
/// '{"command": "ping"}\n'
/// ```
/// To which the server will respond:
/// ```text
/// '{"command": "ping", "status": 200}\n'
/// ```
/// To indicate the command succeeded. Extra data required for most commands will be supplied as additional
/// properties in the same json string. For example, a login packet may look like this:
/// ```text
/// {"command": "login", "uname": "User1", "passwd": "12345"}
/// ```
/// To which the server may respond
/// ```text
/// {"command": "login", "status": 403}
/// ```
/// To indicate the password is incorrect.
///
/// ## Events
/// The server may send a subset of the possible reply packets without a corresponding request. These can be
/// thought of as events, where the server is notifying the client of a change that said client did not initiate
/// itself. For example, if a new user logs in, all other clients will automatically be sent an "online" packet
/// with the new list of online clients.
#[enum_dispatch]
#[derive(Deserialize)]
#[serde(tag = "command")]
#[rustfmt::skip]
pub enum Requests {
    #[serde(rename = "register")]         RegisterRequest,
    #[serde(rename = "login")]            LoginRequest,
    #[serde(rename = "ping")]             PingRequest,
    #[serde(rename = "nick")]             NickRequest,
    #[serde(rename = "online")]           OnlineRequest,
    #[serde(rename = "send")]             SendRequest,
    #[serde(rename = "get_metadata")]     GetMetadataRequest,
    #[serde(rename = "get_name")]         GetNameRequest,
    #[serde(rename = "get_icon")]         GetIconRequest,
    #[serde(rename = "list_emoji")]       ListEmojiRequest,
    #[serde(rename = "get_emoji")]        GetEmojiRequest,
    #[serde(rename = "list_channels")]    ListChannelsRequest,
    #[serde(rename = "history")]          HistoryRequest,
    #[serde(rename = "pfp")]              PfpRequest,
    #[serde(rename = "sync_set")]         SyncSetRequest,
    #[serde(rename = "sync_get")]         SyncGetRequest,
    #[serde(rename = "sync_set_servers")] SyncSetServersRequest,
    #[serde(rename = "sync_get_servers")] SyncGetServersRequest,
    #[serde(rename = "leave")]            LeaveRequest,
    #[serde(rename = "get_user")]         GetUserRequest,
    #[serde(rename = "edit")]             EditRequest,
    #[serde(rename = "delete")]           DeleteRequest,
    #[serde(rename = "change_password")]  PasswordChangeRequest,
    #[serde(rename = "create_channel")]   CreateChannelRequest,
    #[serde(rename = "delete_channel")]   DeleteChannelRequest,
    #[serde(rename = "update_channel")]   UpdateChannelRequest,
    #[serde(rename = "create_group")]     CreateGroupRequest,
    #[serde(rename = "delete_group")]     DeleteGroupRequest,
    #[serde(rename = "update_group")]     UpdateGroupRequest,
    #[serde(rename = "update_user_groups")] UpdateUserGroupsRequest,
    #[serde(rename = "list_groups")]      ListGroupsRequest,

    #[serde(rename = "get_last_reads")]   GetLastReadsRequest,
    #[serde(rename = "mark_unread")]      MarkUnreadRequest,
    #[serde(rename = "get_num_unread")]   GetNumUnreadRequest,
}

#[derive(Serialize)]
#[serde(tag = "command")]
#[rustfmt::skip]
pub enum Response {  
    #[serde(rename = "API_version")]      APIVersionResponse { version: [u8; 3] },
    #[serde(rename = "register")]         RegisterResponse { uuid: i64 },
    #[serde(rename = "login")]            LoginResponse { uuid: i64 },
    #[serde(rename = "get_metadata")]     GetMetadataResponse { data: Vec<User> },
    #[serde(rename = "sync_get_servers")] SyncGetServersResponse { servers: Vec<SyncServer> },
    #[serde(rename = "online")]           OnlineResponse { data: Vec<i64> },
    #[serde(rename = "history")]          HistoryResponse { data: Vec<Message> },
    #[serde(rename = "get_user")]         GetUserResponse { data: User },
    #[serde(rename = "get_icon")]         GetIconResponse { data: String },
    #[serde(rename = "get_name")]         GetNameResponse { data: String },
    #[serde(rename = "list_channels")]    ListChannelsResponse { data: Vec<Channel> },
    #[serde(rename = "get_emoji")]        GetEmojiResponse { data: Emoji },
    #[serde(rename = "list_emoji")]       ListEmojiResponse { data: Vec<(String, i64)> },
    #[serde(rename = "send")]             SendResponse { message: Uuid },
    #[serde(rename = "message_edited")]   MessageEditedResponse { message: Uuid, new_content: String },
    #[serde(rename = "message_deleted")]  MessageDeletedResponse { message: Uuid },
    #[serde(rename = "list_groups")]      ListGroupsResponse { data: Vec<Group> },
    #[serde(rename = "create_channel")]   CreateChannelResponse { uuid: Uuid },

    #[serde(rename = "get_last_reads")]   GetLastReadsResponse { last_reads: HashMap<Uuid, i64> },
    #[serde(rename = "get_num_unread")]   GetNumUnreadResponse { num: u32 },

    #[serde(rename = "content")]
    ContentResponse {
        #[serde(flatten)]
        message: Message,
    },


    #[serde(rename = "sync_get")]
    SyncGetResponse {
        #[serde(flatten)]
        data: SyncData,
    },

    // if any command produces an error, it will not need to return any data
    // thus, to avoid having all data be Option<T>, define a generic response to just include a status
    GenericResponse(Status),
}

type CmdError = anyhow::Error;
use Response::*;

// This is over-engineered
trait HasOrder {
    fn pos(&self) -> usize;
    fn with_pos(self, pos: usize) -> Self;
}

impl HasOrder for Channel {
    fn pos(&self) -> usize {
        self.position
    }
    fn with_pos(self, pos: usize) -> Self {
        Self {
            uuid: self.uuid,
            name: self.name,
            position: pos,
            permissions: self.permissions,
        }
    }
}
impl HasOrder for Group {
    fn pos(&self) -> usize {
        self.position
    }
    fn with_pos(self, pos: usize) -> Self {
        Self {
            uuid: self.uuid,
            permissions: self.permissions,
            name: self.name,
            colour: self.colour,
            position: pos,
        }
    }
}

fn moveto<T: HasOrder, F: FnMut(T) -> Result<(), DbError>>(
    from: usize,
    to: usize,
    things: Vec<T>,
    mut update_thing: F,
) -> Result<(), DbError> {
    // recalculate channel positions, given the position of this channel
    // TODO maybe use a transaction...
    if from == to {
        return Ok(());
    }

    if from < to {
        for c in things {
            if c.pos() == from {
                update_thing(c.with_pos(to))?;
            } else if c.pos() > from && c.pos() <= to {
                let pos = c.pos();
                update_thing(c.with_pos(pos - 1))?;
            }
        }
    } else if from > to {
        for c in things {
            if c.pos() == from {
                update_thing(c.with_pos(to))?;
            } else if c.pos() >= to && c.pos() < from {
                let pos = c.pos();
                update_thing(c.with_pos(pos + 1))?;
            }
        }
    }

    Ok(())
}

#[enum_dispatch(Requests)]
pub trait Request {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError>;
}

fn get_viewable_channels(
    state_lock: &LockedState,
    all_channels: &[Channel],
    user: &User,
) -> Result<Vec<Channel>, CmdError> {
    let mut our_channels = Vec::new();
    for channel in all_channels {
        if state_lock
            .resolve_channel_permissions(user, channel)?
            .view_channel
            == Perm::Allow
        {
            our_channels.push(channel.clone());
        }
    }
    Ok(our_channels)
}

fn update_channels(state_lock: &mut LockedState) -> Result<(), CmdError> {
    let channels = state_lock.get_channels()?;
    for (tx, _, uuid) in state_lock.peers.iter() {
        if let Some(uuid) = uuid {
            let user = state_lock.get_user_exists(*uuid)?;
            let our_channels = get_viewable_channels(state_lock, &channels, &user)?;
            let mut packet = serde_json::to_value(ListChannelsResponse { data: our_channels })?;
            packet["status"] = (Status::Ok as i32).into();
            tx.send(packet.clone())?;
        }
    }
    Ok(())
}

fn update_groups(state_lock: &mut LockedState) -> Result<(), CmdError> {
    let groups = state_lock.get_groups()?;
    let mut packet = serde_json::to_value(ListGroupsResponse { data: groups })?;
    packet["status"] = (Status::Ok as i32).into();
    state_lock.send_to_all(packet)?;
    Ok(())
}

pub fn server_perms(state_lock: &LockedState, peer: &Peer) -> Result<Permissions, DbError> {
    let user = state_lock.get_user(peer.uuid.unwrap())?.unwrap();
    state_lock.resolve_server_permissions(&user)
}
pub fn channel_perms(
    state_lock: &LockedState,
    uuid: Option<Uuid>,
    channel: &Channel,
) -> Result<Permissions, DbError> {
    let user = state_lock.get_user(uuid.unwrap())?.unwrap();
    state_lock.resolve_channel_permissions(&user, channel)
}

impl Request for GetLastReadsRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        let Some(user_uuid) = peer.uuid else {
            return Ok(GenericResponse(Status::Unauthenticated));
        };

        let last_reads = state_lock.get_last_read_messages(user_uuid)?;

        Ok(GetLastReadsResponse { last_reads })
    }
}

impl Request for GetNumUnreadRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        let Some(user_uuid) = peer.uuid else {
            return Ok(GenericResponse(Status::Unauthenticated));
        };

        let num = state_lock.get_num_unread_messages(user_uuid, self.channel)?;

        Ok(GetNumUnreadResponse { num })
    }
}

impl Request for MarkUnreadRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        let Some(user_uuid) = peer.uuid else {
            return Ok(GenericResponse(Status::Unauthenticated));
        };

        let Some(message) = state_lock.get_message(self.uuid)? else {
            return Ok(GenericResponse(Status::NotFound));
        };

        state_lock.update_last_read_for_user_in_channel(
            user_uuid,
            message.channel_uuid,
            message.uuid,
        )?;

        Ok(GenericResponse(Status::Ok))
    }
}

impl Request for UpdateUserGroupsRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Unauthenticated));
        }
        if server_perms(state_lock, peer)?.modify_user_groups != Perm::Allow {
            return Ok(GenericResponse(Status::Forbidden));
        }

        let Some(mut user) = state_lock.get_user(self.user)? else {
            return Ok(GenericResponse(Status::NotFound));
        };

        for g in &self.groups {
            if state_lock.get_group(*g)?.is_none() {
                return Ok(GenericResponse(Status::NotFound));
            }
        }

        user.groups = self.groups;
        state_lock.update_user(&user)?;

        Ok(GenericResponse(Status::Ok))
    }
}

impl Request for CreateGroupRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Unauthenticated));
        }
        if server_perms(state_lock, peer)?.modify_groups != Perm::Allow {
            return Ok(GenericResponse(Status::Forbidden));
        }

        // forbid altering higher groups
        let highest_role = state_lock.get_highest_group_pos_of(peer.uuid.unwrap())?;
        if self.position <= highest_role {
            return Ok(GenericResponse(Status::Forbidden));
        }

        let groups = state_lock.get_groups()?;

        // Cannot add a group (too far) past the end of existing groups
        // TODO could this just be a last() cos they are in the right order
        let next_position = groups.iter().map(|g| g.position + 1).max().unwrap_or(0);
        if self.position > next_position {
            return Ok(GenericResponse(Status::BadRequest));
        }

        state_lock.insert_group(&Group {
            uuid: gen_uuid(),
            permissions: self.permissions,
            name: self.name,
            colour: self.colour,
            position: next_position,
        })?;

        moveto(next_position, self.position, groups, |g| {
            state_lock.update_group(&g)
        })?;

        update_groups(state_lock)?;
        Ok(GenericResponse(Status::Ok))
    }
}
impl Request for DeleteGroupRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Unauthenticated));
        }
        if server_perms(state_lock, peer)?.modify_groups != Perm::Allow {
            return Ok(GenericResponse(Status::Forbidden));
        }

        let Some(group) = state_lock.get_group(self.uuid)? else {
            return Ok(GenericResponse(Status::NotFound));
        };

        // forbid altering higher groups
        let highest_group = state_lock.get_highest_group_pos_of(peer.uuid.unwrap())?;
        if group.position <= highest_group {
            return Ok(GenericResponse(Status::Forbidden));
        }

        state_lock.delete_group(self.uuid)?;
        // shift down the groups
        for g in state_lock.get_groups()? {
            if g.position > group.position {
                let new_pos = g.position - 1;
                state_lock.update_group(&g.with_pos(new_pos))?;
            }
        }

        update_groups(state_lock)?;
        Ok(GenericResponse(Status::Ok))
    }
}
impl Request for UpdateGroupRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Unauthenticated));
        }
        if server_perms(state_lock, peer)?.modify_groups != Perm::Allow {
            return Ok(GenericResponse(Status::Forbidden));
        }

        let Some(old) = state_lock.get_group(self.uuid)? else {
            return Ok(GenericResponse(Status::NotFound));
        };

        let position = self.position.unwrap_or(old.position);
        let name = self.name.unwrap_or(old.name);
        let permissions = self.permissions.unwrap_or(old.permissions);
        let colour = self.colour.unwrap_or(old.colour);

        let new_group = Group {
            uuid: self.uuid,
            position,
            name,
            permissions,
            colour,
        };

        // forbid altering higher groups
        let highest_role = state_lock.get_highest_group_pos_of(peer.uuid.unwrap())?;
        if position <= highest_role || old.position <= highest_role {
            return Ok(GenericResponse(Status::Forbidden));
        }

        let groups = state_lock.get_groups()?;
        moveto(old.position, position, groups, |g| {
            state_lock.update_group(&g)
        })?;

        state_lock.update_group(&new_group)?;
        todo!();
    }
}

impl Request for CreateChannelRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Unauthenticated));
        }
        if server_perms(state_lock, peer)?.modify_channels != Perm::Allow {
            return Ok(GenericResponse(Status::Forbidden));
        }
        // TODO consider forbidding altering perms for higher groups
        // TODO consider forbidding altering perms if no modify_groups perm

        let channels = state_lock.get_channels()?;

        // Cannot add a channel (too far) past the end of existing channels
        let next_position = channels.iter().map(|c| c.position + 1).max().unwrap_or(0);
        let position = self.position.unwrap_or(next_position);
        if position > next_position {
            return Ok(GenericResponse(Status::BadRequest));
        }

        let uuid = gen_uuid();
        state_lock.insert_channel(&Channel {
            uuid,
            name: self.name,
            permissions: HashMap::new(),
            position: next_position,
        })?;

        moveto(next_position, position, channels, |channel| {
            state_lock.update_channel(&channel)
        })?;
        update_channels(state_lock)?;
        Ok(CreateChannelResponse { uuid })
    }
}
impl Request for DeleteChannelRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Unauthenticated));
        }
        if server_perms(state_lock, peer)?.modify_channels != Perm::Allow {
            return Ok(GenericResponse(Status::Forbidden));
        }

        let Some(channel) = state_lock.get_channel(&self.channel)? else {
            return Ok(GenericResponse(Status::NotFound));
        };

        state_lock.delete_channel(self.channel)?;
        // shift down the channels
        for c in state_lock.get_channels()? {
            if c.position > channel.position {
                let new_pos = c.position - 1;
                state_lock.update_channel(&c.with_pos(new_pos))?;
            }
        }

        update_channels(state_lock)?;
        Ok(GenericResponse(Status::Ok))
    }
}
impl Request for UpdateChannelRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Unauthenticated));
        }
        if server_perms(state_lock, peer)?.modify_channels != Perm::Allow {
            return Ok(GenericResponse(Status::Forbidden));
        }
        // TODO consider forbidding altering perms for higher groups
        // TODO consider forbidding altering perms if no modify_groups perm
        let channels = state_lock.get_channels()?;
        let Some(old_channel) = state_lock.get_channel(&self.uuid)? else {
            return Ok(GenericResponse(Status::NotFound));
        };

        let position = self.position.unwrap_or(old_channel.position);
        let name = self.name.unwrap_or(old_channel.name);
        let permissions = self.permissions.unwrap_or(old_channel.permissions);
        let new_channel = Channel {
            uuid: self.uuid,
            name,
            permissions,
            position,
        };

        // unwrap ok, because we know there must be at least one channel as it exists
        if position > channels.last().unwrap().position {
            return Ok(GenericResponse(Status::BadRequest));
        }

        moveto(old_channel.position, position, channels, |channel| {
            state_lock.update_channel(&channel)
        })?;

        state_lock.update_channel(&new_channel)?;

        update_channels(state_lock)?;
        Ok(GenericResponse(Status::Ok))
    }
}

fn send_metadata(state_lock: &mut LockedState, peer: &Peer) {
    if let Some(uuid) = peer.uuid {
        match state_lock.get_user(uuid) {
            Ok(Some(peer_meta)) => {
                let meta = peer_meta;
                let result =
                    json!({"command": "get_metadata", "data": [meta], "status": Status::Ok as i32});
                state_lock.send_to_all(result).unwrap(); //TODO get rid of this unwrap
            }
            Ok(None) => log::warn!("send_metadata: Requested peer metadata not found: {}", uuid),
            Err(e) => log::error!(
                "send_metadata: Requested peer metadata returned error: {:?}",
                e
            ),
        }
    }
}

pub fn count_online(state_lock: &LockedState) -> Vec<i64> {
    state_lock
        .online
        .iter()
        .filter(|a| *a.1 > 0)
        .map(|a| *a.0)
        .collect()
}

pub fn send_online(state_lock: &LockedState) {
    let num_online = count_online(state_lock);

    let mut final_json = serde_json::to_value(OnlineResponse { data: num_online }).unwrap(); // unwrap ok because OnlineResponse derives Serialize, and it does not contain any maps
    final_json["status"] = (Status::Ok as i32).into(); // to make sure the client doesn't panic...
    state_lock.send_to_all(final_json).unwrap(); //TODO get rid of this unwrap
}

fn execute_request(
    request: Requests,
    state: &mut LockedState,
    peer: &mut Peer,
    command: &str,
) -> JsonValue {
    match request.execute(state, peer) {
        Ok(response) => {
            // generic response is just a status: we send back the command that the client sent
            if let Response::GenericResponse(status) = response {
                json!({"status": status as i32, "command": command})
            } else {
                let mut response_json = serde_json::to_value(response).unwrap();
                if !response_json["status"].is_number() {
                    // if the response doesn't define a status, assume it's Ok (200)
                    // since basically all of the non-ok statuses are handled by GenericResponse
                    response_json["status"] = (Status::Ok as i32).into();
                }
                response_json
            }
        }
        Err(e) => {
            log::error!("In command '{}', internal error {:?}", command, e,);
            json!({"status": Status::InternalError as i32, "command": command})
        }
    }
}

pub fn process_command(
    msg: &str,
    state: &mut LockedState,
    peer: &mut Peer,
) -> Result<(), Box<dyn Error>> {
    let a = std::time::Instant::now();
    let response = match serde_json::from_str::<JsonValue>(msg) {
        Ok(raw_request) => {
            let command = if raw_request["command"].is_string() {
                raw_request["command"].as_str().unwrap().to_owned()
            } else {
                "unknown".to_owned()
            };
            print!("Request {command}");

            match serde_json::from_value::<Requests>(raw_request) {
                Ok(request) => execute_request(request, state, peer, &command),
                Err(_) => {
                    json!({"command": command, "status": Status::BadRequest as i32})
                }
            }
        }
        Err(_) => {
            json!({"command": "unknown", "status": Status::BadRequest as i32})
        }
    };
    // println!("Got request '{}' and responded with '{:?}'", msg, response);
    peer.tx.send(response)?;
    let d = a.elapsed();
    println!(" took {}µs to respond", d.as_micros());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        helper::{gen_uuid, Uuid},
        models::Channel,
    };

    use super::moveto;

    #[test]
    fn reorder_channels() {
        let mut ch_db: HashMap<Uuid, Channel> = HashMap::new();
        for i in 0..10 {
            let uuid = gen_uuid();
            ch_db.insert(
                uuid,
                Channel {
                    uuid,
                    name: format!("{}", i),
                    position: i,
                    permissions: HashMap::new(),
                },
            );
        }

        let channels_vec: Vec<Channel> = ch_db.values().map(|c| c.clone()).collect();
        moveto(3, 6, channels_vec.clone(), |thing: Channel| {
            ch_db.insert(thing.uuid, thing);
            Ok(())
        })
        .unwrap();
        let channels_vec: Vec<Channel> = ch_db.values().map(|c| c.clone()).collect();
        moveto(7, 2, channels_vec.clone(), |thing: Channel| {
            ch_db.insert(thing.uuid, thing);
            Ok(())
        })
        .unwrap();
        let channels_vec: Vec<Channel> = ch_db.values().map(|c| c.clone()).collect();
        moveto(5, 5, channels_vec.clone(), |thing: Channel| {
            ch_db.insert(thing.uuid, thing);
            Ok(())
        })
        .unwrap();
        let mut new_channels: Vec<Channel> = ch_db.values().map(|c| c.clone()).collect();
        new_channels.sort_by_key(|c| c.position);
        println!(
            "{}",
            new_channels
                .iter()
                .map(|c| format!("{}: {}", c.position, c.name))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(
            new_channels.iter().map(|c| c.position).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
        assert_eq!(
            new_channels.iter().map(|c| &c.name).collect::<Vec<_>>(),
            vec!["0", "1", "7", "2", "4", "5", "6", "3", "8", "9",]
        );
    }
}
