mod auth;
mod log_any;
mod log_in;
mod log_out;

//use auth::*;
use log_any::*;
//use log_in::*;
use log_out::*;

use crate::helper::{gen_uuid, JsonValue, LockedState};
use crate::models::{SyncData, SyncServer, SyncServerQuery, User};
use crate::message::Message;
use crate::peer::Peer;
use crate::schema;
use crate::shared::Shared;

use diesel::prelude::*;
use enum_dispatch::enum_dispatch;
use serde::{Serialize, Deserialize, Serializer};
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Copy)]
pub enum Status {
    Ok = 200,
    BadRequest = 400,
    InternalError = 500,
    Unauthorised = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
}

impl Serialize for Status {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(*self as i32)
    }
}

#[derive(Deserialize)]
pub struct SendRequest {
    pub content: String,
    pub channel: i64,
}

#[derive(Deserialize)]
pub struct HistoryRequest {
    pub num: u32,
    pub channel: i64,
}

#[derive(Deserialize)]
pub struct SyncSetRequest {
    pub uname: String,
    pub pfp: String,
}

#[derive(Deserialize)]
pub struct SyncSetServersRequest {
    pub servers: Vec<SyncServer>,
}

#[derive(Deserialize)]
pub struct NickRequest {
    pub nick: String,
}
#[derive(Deserialize)]
pub struct PfpRequest {
    pub data: String,
}
#[derive(Deserialize)] pub struct SyncGetRequest;
#[derive(Deserialize)] pub struct SyncGetServersRequest;
#[derive(Deserialize)] pub struct LeaveRequest;
#[derive(Deserialize)] pub struct OnlineRequest;
#[derive(Deserialize)] pub struct PingRequest;

#[derive(Serialize)]
pub struct RegisterResponse {
    pub status: Status,
    pub uuid: Option<i64>,
}

#[derive(Serialize)]
pub struct MetadataResponse {
    pub status: Status,
    pub data: Vec<User>,
}

#[derive(Serialize)]
pub struct OnlineResponse {
    pub status: Status,
    pub data: Vec<i64>,
}

#[derive(Serialize)]
pub struct LeaveResponse {
    pub status: Status,
}

#[derive(Serialize)]
pub struct PingResponse {
    pub status: Status,
}

#[derive(Serialize)]
pub struct NickResponse {
    pub status: Status,
}



#[derive(Serialize)]
#[serde(tag = "command")]
enum Response {
    #[serde(rename = "register")] RegisterResponse,
    #[serde(rename = "metadata")] MetadataResponse,
    #[serde(rename = "online")]   OnlineResponse,
    #[serde(rename = "leave")]    LeaveResponse,
    #[serde(rename = "ping")]     PingResponse,
    #[serde(rename = "nick")]     NickResponse,
}

#[enum_dispatch]
#[derive(Deserialize)]
#[serde(tag = "command")]
enum Request {
    #[serde(rename = "register")]      RegisterRequest,
    #[serde(rename = "login")]         LoginRequest,
    #[serde(rename = "ping")]          PingRequest,
    #[serde(rename = "nick")]          NickRequest,
    #[serde(rename = "online")]        OnlineRequest,
    #[serde(rename = "send")]          SendRequest,
    #[serde(rename = "metadata")]      GetMetadataRequest,
    #[serde(rename = "get_name")]      GetNameRequest,
    #[serde(rename = "get_icon")]      GetIconRequest,
    #[serde(rename = "list_emoji")]    ListEmojiRequest,
    #[serde(rename = "get_emoji")]     GetEmojiRequest,
    #[serde(rename = "list_channels")] ListChannelsRequest,
    #[serde(rename = "history")]       HistoryRequest,
    #[serde(rename = "pfp")]           PfpRequest,
    #[serde(rename = "sync_set")]      SyncSetRequest,
    #[serde(rename = "sync_get")]      SyncGetRequest,
    #[serde(rename = "sync_set_servers")] SyncSetServersRequest,
    #[serde(rename = "sync_get_servers")] SyncGetServersRequest,
    #[serde(rename = "leave")]         LeaveRequest,
    #[serde(rename = "get_user")]      GetUserRequest,
}

#[enum_dispatch(Request)]
pub trait Packet {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue;
}

fn send_metadata(state_lock: &mut LockedState, peer: &Peer) {
    
    match state_lock.get_user(&peer.user) {
        Ok(Some(peer_meta)) => {
            let meta = json!([serde_json::to_value(peer_meta).unwrap()]);
            let result = json!({"command": "metadata", "data": meta, "status": Status::Ok as i32});
            state_lock.send_to_all(result);
        },
        Ok(None) => println!("Warn(send_metadata): Requested peer metadata not found: {}", peer.user),
        Err(e) => println!("Error(send_metadata): Requested peer metadata returned error: {:?}", e),
    };

}

pub fn send_online(state_lock: &LockedState) {
    let mut res = Vec::new();
    for user in state_lock.online.iter().filter(|a| *a.1 > 0) {
        res.push(json!(*user.0));
    }
    let final_json = json!({
        "command": "online",
        "data": res,
        "status": Status::Ok as i32,
    });
    state_lock.send_to_all(final_json);
}

impl Packet for LeaveRequest {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> JsonValue {
        json!({"command": "leave", "status": Status::Ok as i32})
    }
}

impl Packet for PingRequest {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> JsonValue {
        json!({"command": "ping", "status": Status::Ok as i32})
    }
}

impl Packet for NickRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "nick", "status": Status::Forbidden as i32});
        }

        match state_lock.get_user(&peer.user) {
            Ok(Some(mut user)) => {
                user.name = self.nick.to_string();

                if let Err(e) = state_lock.update_user(user) {
                    println!("Error(NickPacket): updating user: {}", e);
                    return json!({"command": "nick", "status": Status::InternalError as i32});
                }
                send_metadata(state_lock, peer);
                json!({"command": "nick", "status": Status::Ok as i32})
            },
            Ok(None) => json!({"command": "nick", "status": Status::NotFound as i32}),
            Err(e) => {
                println!("Error(NickPacket::execute): Error getting user: {:?}", e);
                json!({"command": "nick", "status": Status::InternalError as i32})
            }
        }
    }
}

impl Packet for OnlineRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "online", "status": Status::Forbidden as i32});
        }

        json!({
            "command": "online",
            "data": state_lock.online.clone(),
            "status": Status::Ok as i32,
        })
    }
}

impl Packet for SendRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "send", "status": Status::Forbidden as i32});
        }
        let msg = Message {
            uuid: gen_uuid(),
            content: self.content.to_owned(),
            author_uuid: peer.user,
            channel_uuid: self.channel,
            date: chrono::offset::Utc::now().timestamp() as i32,
        };
        state_lock.add_to_history(&msg);
        match serde_json::to_value(&msg) {
            Ok(mut msg_json) => {
                msg_json["command"] = "content".into();
                msg_json["status"] = (Status::Ok as i32).into();
                state_lock.send_to_all(msg_json);
                json!({"command": "send", "status": Status::Ok as i32})
            },
            Err(e) => {
                println!("Error(SendPacket::execute): Converting message to json failed: {}", e);
                json!({"command": "send", "status": Status::InternalError as i32})
            }
        }
    }
}

impl Packet for HistoryRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "history", "status": Status::Forbidden as i32});
        }
        // check the channel exists
        if let Ok(channel) = state_lock.get_channel(&self.channel) {
            if channel.is_none() {
                return json!({"command": "history", "status": Status::NotFound as i32});
            }
        } else {
            return json!({"command": "history", "status": Status::InternalError as i32});
        }
        match schema::messages::table
            .filter(schema::messages::channel_uuid.eq(self.channel))
            .order(schema::messages::date.desc())
            .limit(self.num.into())
            .load::<Message>(&mut state_lock.conn)
        {
            Ok(mut history) => {
                history.reverse();
                json!({"command": "history", "data": history, "status": Status::Ok as i32})
            }
            Err(e) => {
                println!(
                    "Error(HistoryPacket::execute) error loading database: {:?}",
                    e
                );
                json!({"command": "history", "status": Status::InternalError as i32})
            }
        }
    }
}

impl Packet for PfpRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "pfp", "status": Status::Forbidden as i32});
        }
        match state_lock.get_user(&peer.user) {
            Ok(Some(mut user)) => {
                user.pfp = self.data.to_string();

                if let Err(e) = state_lock.update_user(user) {
                    println!("Error(pfpPacket): updating user: {}", e);
                    return json!({"command": "pfp", "status": Status::InternalError as i32});
                }
                send_metadata(state_lock, peer);
                json!({"command": "pfp", "status": Status::Ok as i32})
            },
            Ok(None) => json!({"command": "pfp", "status": Status::NotFound as i32}),
            Err(e) => {
                println!("Error(PfpPacket::execute): Error getting user: {:?}", e);
                json!({"command": "pfp", "status": Status::InternalError as i32})
            }
        }
    }
}

impl Packet for SyncSetRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "sync_set", "status": Status::Forbidden as i32});
        }

        let mut sync_data = match state_lock.get_sync_data(&peer.user) {
            Ok(Some(data)) => data,
            Ok(None) => {
                let data = SyncData::new(peer.user);
                if let Err(e) = state_lock.insert_sync_data(&data) {
                    println!("Error(SyncSetPacket::execute) error inserting new sync data: {:?}", e);
                    return json!({"command": "sync_set", "status": Status::InternalError as i32});
                }
                data
            },
            Err(e) => {
                println!("Error(SetSyncPacket::execute) error getting sync data: {:?}", e);
                return json!({"command": "set_sync", "status": Status::InternalError as i32});
            }
        };

        sync_data.uname = self.uname.clone();
        sync_data.pfp = self.pfp.clone();

        match state_lock.update_sync_data(sync_data) {
            Ok(_) => {
                json!({"command": "sync_set", "status": Status::Ok as i32})
            }
            Err(e) => {
                println!(
                    "Warn(SyncSetPacket::execute) error updating sync data: {:?}",
                    e
                );
                json!({"command": "sync_set", "status": Status::InternalError as i32})
            }
        }
    }
}

impl Packet for SyncGetRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "sync_get", "status": Status::Forbidden as i32});
        }

        let sync_data = state_lock.get_sync_data(&peer.user);
        match sync_data {
            Ok(Some(sync_data)) => 
                json!({"command": "sync_get", 
                   "uname": sync_data.uname.as_str(),
                   "pfp": sync_data.pfp.as_str(),
                   "status": Status::Ok as i32}),
            Ok(None) =>
                json!({"command": "sync_get", "status": Status::NotFound as i32}),
            Err(e) => {
                println!("Warn(SyncGetPacket::execute) error getting sync data: {:?}", e);
                json!({"command": "sync_get", "status": Status::InternalError as i32})
            }
        }
    }
}
impl Packet for SyncSetServersRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "sync_set_servers", "status": Status::Forbidden as i32});
        }

        diesel::delete(
            schema::sync_servers::table.filter(schema::sync_servers::user_uuid.eq(peer.user)))
            .execute(&mut state_lock.conn)
            .unwrap();

        for (idx, sync_server) in self.servers.iter().enumerate() {
            let mut server = sync_server.clone();
            server.user_uuid = peer.user;
            server.idx = idx as i32;
            if let Err(e) = state_lock.insert_sync_server(server) {
                println!(
                    "Warn(SyncSetServersPacket::execute) error setting sync server: {:?}",
                    e
                );
                return json!({"command": "sync_get_servers", "status": Status::InternalError as i32});
            }
        }

        json!({"command": "sync_set_servers", "status": Status::Ok as i32})
    }
}

impl Packet for SyncGetServersRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "sync_get_servers", "status": Status::Forbidden as i32});
        }
        let servers = schema::sync_servers::table
            .filter(schema::sync_servers::user_uuid.eq(peer.user))
            .order(schema::sync_servers::idx.asc())
            .load::<SyncServerQuery>(&mut state_lock.conn);

        match servers {
            Ok(servers) => {
                let servers = servers
                    .into_iter()
                    .map(SyncServer::from)
                    .collect::<Vec<SyncServer>>();
                json!({"command": "sync_get_servers",
                       "servers": servers,
                       "status": Status::Ok as i32})
            }
            Err(e) => {
                println!(
                    "Warn(SyncGetServersPacket::execute) error getting sync servers: {:?}",
                    e
                );
                json!({"command": "sync_set_servers", "status": Status::InternalError as i32})
            }
        }
    }
}

pub async fn process_command(
    msg: &String,
    state: Arc<Mutex<Shared>>,
    peer: &mut Peer,
) -> Result<(), Box<dyn Error>> {
    let response = match serde_json::from_str::<Request>(msg) {
        Ok(request) => {
            let mut state_lock = state.lock().await;
            request.execute(&mut state_lock, peer)
        }
        Err(e) => {
            println!(
                "Warn(process_command) error decoding packet '{}': {:?}",
                msg, e
            );
            json!({"command": "unknown", "status": Status::BadRequest as i32})
        }
    };
    peer.tx.send(response)?;
    /*
            //"/createchannel" => {
            //
            //    shared_lock.channels.insert("#".to_string(), SharedChannel::new());
            //}

            "/delete" => {
                //TODO what
                let uuid = argv[1].parse::<i64>().unwrap();
                diesel::delete(schema::users::table.filter(schema::users::uuid.eq(uuid))).execute(&state_lock.conn).unwrap();
                diesel::delete(schema::messages::table.filter(
                    schema::messages::author_uuid.eq(uuid))).execute(&state_lock.conn).unwrap();
            }
            _ => ()
        }
    } else {
        json::object!{command: "unknown", status: Status::BadRequest as i32}
    };*/
    Ok(())
}
