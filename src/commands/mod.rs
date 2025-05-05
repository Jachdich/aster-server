mod auth;
mod log_any;
mod log_in;
mod log_out;

//use auth::*;
use log_any::*;
use log_in::*;
use log_out::*;

use crate::helper::{gen_uuid, JsonValue, LockedState, Uuid};
use crate::message::Message;
use crate::peer::Peer;

use crate::models::{Channel, Emoji, SyncData, SyncServer, User};
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::json;
use std::error::Error;

#[derive(Clone, Copy, Debug)]
pub enum Status {
    Ok = 200,
    BadRequest = 400,
    InternalError = 500,
    Unauthorised = 401,
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
}

#[derive(Serialize)]
#[serde(tag = "command")]
#[rustfmt::skip]
pub enum Response {  
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

#[enum_dispatch(Requests)]
pub trait Request {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError>;
}

fn update_channels(state_lock: &mut LockedState) -> Result<(), CmdError> {
    let channels = state_lock.get_channels()?;
    let mut packet = serde_json::to_value(ListChannelsResponse { data: channels })?;
    packet["status"] = (Status::Ok as i32).into();
    state_lock.send_to_all(packet)?;
    Ok(())
}

impl Request for CreateChannelRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }
        state_lock.insert_channel(Channel { uuid: gen_uuid(), name: self.channel_name })?;
        update_channels(state_lock)?;
        Ok(GenericResponse(Status::Ok))
    }
}
impl Request for DeleteChannelRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }
        state_lock.delete_channel(self.channel)?;
        update_channels(state_lock)?;
        Ok(GenericResponse(Status::Ok))
    }
}
impl Request for UpdateChannelRequest {
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }
        update_channels(state_lock)?;
        Ok(GenericResponse(Status::Ok))
    }
}

fn send_metadata(state_lock: &mut LockedState, peer: &Peer) {
    if let Some(uuid) = peer.uuid {
        match state_lock.get_user(&uuid) {
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
            print!("Request {}", command);

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
