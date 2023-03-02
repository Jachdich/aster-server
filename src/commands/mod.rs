mod auth;
mod log_any;
mod log_in;
mod log_out;

//use auth::*;
use log_any::*;
//use log_in::*;
use log_out::*;

use crate::helper::{gen_uuid, JsonValue, LockedState};
use crate::models::{SyncData, SyncServer, SyncServerQuery};
use crate::message::Message;
use crate::peer::Peer;
use crate::schema;
use crate::shared::Shared;

use diesel::prelude::*;
use enum_dispatch::enum_dispatch;
use futures::SinkExt;
use serde::Deserialize;
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

pub enum Status {
    Ok = 200,
    BadRequest = 400,
    InternalError = 500,
    Unauthorised = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
}

#[derive(Deserialize)]
pub struct SendPacket {
    pub content: String,
    pub channel: i64,
}
#[derive(Deserialize)]
pub struct HistoryPacket {
    pub num: u32,
    pub channel: i64,
}

#[derive(Deserialize)]
pub struct SyncSetPacket {
    pub uname: String,
    pub pfp: String,
}

#[derive(Deserialize)]
pub struct SyncSetServersPacket {
    pub servers: Vec<SyncServer>,
}

#[derive(Deserialize)]
pub struct PingPacket;
#[derive(Deserialize)]
pub struct NickPacket {
    pub nick: String,
}
#[derive(Deserialize)]
pub struct OnlinePacket;
#[derive(Deserialize)]
pub struct PfpPacket {
    pub data: String,
}
#[derive(Deserialize)]
pub struct SyncGetPacket;
#[derive(Deserialize)]
pub struct SyncGetServersPacket;
#[derive(Deserialize)]
pub struct LeavePacket;

#[enum_dispatch]
#[derive(Deserialize)]
#[serde(tag = "command")]
enum Packets {
    #[serde(rename = "register")]
    RegisterPacket,
    #[serde(rename = "login")]
    LoginPacket,
    #[serde(rename = "ping")]
    PingPacket,
    #[serde(rename = "nick")]
    NickPacket,
    #[serde(rename = "online")]
    OnlinePacket,
    #[serde(rename = "send")]
    SendPacket,
    #[serde(rename = "get_metadata")]
    GetMetadataPacket,
    #[serde(rename = "get_name")]
    GetNamePacket,
    #[serde(rename = "get_icon")]
    GetIconPacket,
    #[serde(rename = "list_emoji")]
    ListEmojiPacket,
    #[serde(rename = "get_emoji")]
    GetEmojiPacket,
    #[serde(rename = "list_channels")]
    ListChannelsPacket,
    #[serde(rename = "history")]
    HistoryPacket,
    #[serde(rename = "pfp")]
    PfpPacket,
    #[serde(rename = "sync_set")]
    SyncSetPacket,
    #[serde(rename = "sync_get")]
    SyncGetPacket,
    #[serde(rename = "sync_set_servers")]
    SyncSetServersPacket,
    #[serde(rename = "sync_get_servers")]
    SyncGetServersPacket,
    #[serde(rename = "leave")]
    LeavePacket,
    #[serde(rename = "get_user")]
    GetUserPacket,
}

#[enum_dispatch(Packets)]
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

impl Packet for LeavePacket {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> JsonValue {
        json!({"command": "leave", "status": Status::Ok as i32})
    }
}

impl Packet for PingPacket {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> JsonValue {
        json!({"command": "ping", "status": Status::Ok as i32})
    }
}

impl Packet for NickPacket {
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

impl Packet for OnlinePacket {
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

impl Packet for SendPacket {
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

impl Packet for HistoryPacket {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "history", "status": Status::Forbidden as i32});
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

impl Packet for PfpPacket {
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

impl Packet for SyncSetPacket {
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

impl Packet for SyncGetPacket {
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
impl Packet for SyncSetServersPacket {
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

impl Packet for SyncGetServersPacket {
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
    let response = match serde_json::from_str::<Packets>(msg) {
        Ok(packets) => {
            let mut state_lock = state.lock().await;
            packets.execute(&mut state_lock, peer)
        }
        Err(e) => {
            println!(
                "Warn(process_command) error decoding packet '{}': {:?}",
                msg, e
            );
            json!({"command": "unknown", "status": Status::BadRequest as i32})
        }
    };
    println!("{}", response.to_string());
    peer.lines.send(response.to_string()).await?;
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
