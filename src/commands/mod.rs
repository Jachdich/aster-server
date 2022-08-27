
mod auth;
mod log_any;
mod log_in;
mod log_out;

//use auth::*;
use log_any::*;
//use log_in::*;
use log_out::*;

use crate::helper::{gen_uuid, LockedState, JsonValue};
use crate::shared::Shared;
use crate::peer::Peer;
use crate::message::{CookedMessage, MessageType};
use crate::schema;
use crate::models::{SyncServer, SyncServerQuery, SyncData};

use diesel::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::error::Error;
use futures::SinkExt;
use serde_json::json;
use serde::Deserialize;
use enum_dispatch::enum_dispatch;

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
    pub servers: Vec<SyncServer>
}

#[derive(Deserialize)] pub struct PingPacket;
#[derive(Deserialize)] pub struct NickPacket { pub nick: String }
#[derive(Deserialize)] pub struct OnlinePacket;
#[derive(Deserialize)] pub struct PfpPacket { pub data: String }
#[derive(Deserialize)] pub struct SyncGetPacket;
#[derive(Deserialize)] pub struct SyncGetServersPacket;
#[derive(Deserialize)] pub struct LeavePacket;

#[enum_dispatch]
#[derive(Deserialize)]
#[serde(tag = "command")]
enum Packets {
    #[serde(rename = "register")]      RegisterPacket,
    #[serde(rename = "login")]         LoginPacket,
    #[serde(rename = "ping")]          PingPacket,
    #[serde(rename = "nick")]          NickPacket,
    #[serde(rename = "online")]        OnlinePacket,
    #[serde(rename = "send")]          SendPacket,
    #[serde(rename = "get_metadata")]  GetMetadataPacket,
    #[serde(rename = "get_name")]      GetNamePacket,
    #[serde(rename = "get_icon")]      GetIconPacket,
    #[serde(rename = "list_emoji")]    ListEmojiPacket,
    #[serde(rename = "get_emoji")]     GetEmojiPacket,
    #[serde(rename = "list_channels")] ListChannelsPacket,
    #[serde(rename = "history")]       HistoryPacket,
    #[serde(rename = "pfp")]           PfpPacket,
    #[serde(rename = "sync_set")]      SyncSetPacket,
    #[serde(rename = "sync_get")]      SyncGetPacket,
    #[serde(rename = "sync_set_servers")] SyncSetServersPacket,
    #[serde(rename = "sync_get_servers")] SyncGetServersPacket,  
    #[serde(rename = "leave")]         LeavePacket,
}

#[enum_dispatch(Packets)]
pub trait Packet {
    fn execute(&self,
                   state_lock: &mut LockedState,
                   peer: &mut Peer
    ) -> JsonValue;
}

fn send_metadata(state_lock: &LockedState, peer: &Peer) {
    let meta = json!([serde_json::to_value(state_lock.get_user(&peer.user)).unwrap()]);
    state_lock.send_to_all(MessageType::Raw(json!({"command": "metadata", "data": meta, "status": Status::Ok as i32})));
}

pub fn send_online(state_lock: &LockedState) {
    let mut res = Vec::new();
    for user in state_lock.online.iter() {
        res.push(json!(user));
    }
    let final_json = json!({
        "command": "online",
        "data": res,
        "status": Status::Ok as i32,
    });
    state_lock.send_to_all(MessageType::Raw(final_json));
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

        let mut user = state_lock.get_user(&peer.user);
        user.name = self.nick.to_string();

        match state_lock.update_user(user) {
            Err(_) => return json!({"command": "nick", "status": Status::InternalError as i32}),
            _ => (),
        }

        send_metadata(&state_lock, peer);
        json!({"command": "nick", "status": Status::Ok as i32})
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
        let msg = CookedMessage {
            uuid: gen_uuid(),
            content: self.content.to_owned(),
            author_uuid: peer.user,
            channel_uuid: self.channel,
            date: chrono::offset::Utc::now().timestamp() as i32,
            rowid: 0,
        };
        state_lock.send_to_all(MessageType::Cooked(msg));
        json!({"command": "send", "status": Status::Ok as i32})
    }
}

impl Packet for HistoryPacket {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "history", "status": Status::Forbidden as i32});
        }
        match schema::messages::table
            .filter(schema::messages::channel_uuid.eq(self.channel))
            .order(schema::messages::rowid.desc())
            .limit(self.num.into())
            .load::<CookedMessage>(&state_lock.conn) {
            Ok(mut history) => {
                history.reverse();
                json!({"command": "history", "data": history, "status": Status::Ok as i32})
            }
            Err(e) => {
                println!("Warn(HistoryPacket::execute) error loading database: {:?}", e);
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
        let mut user = state_lock.get_user(&peer.user);
        user.pfp = self.data.to_owned();
        match state_lock.update_user(user) {
            Ok(_) => {
                send_metadata(&state_lock, peer);
                json!({"command": "pfp", "status": Status::Ok as i32})
            },
            Err(e) => {
                println!("Warn(PfpPacket::execute) error updating user: {:?}", e);
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
            Some(data) => data,
            None => {
                let data = SyncData::new(peer.user);
                if let Err(e) = state_lock.insert_sync_data(&data) {
                    println!("Warn(SyncSetPacket) error inserting new sync data: {:?}", e);
                    return json!({"command": "sync_set", "status": Status::Forbidden as i32});
                }
                data
            }
        };

        sync_data.uname = self.uname.clone();
        sync_data.pfp = self.pfp.clone();

        match state_lock.update_sync_data(sync_data) {
            Ok(_) => {
                json!({"command": "sync_set", "status": Status::Ok as i32})
            },
            Err(e) => {
                println!("Warn(SyncSetPacket::execute) error updating sync data: {:?}", e);
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
        if let Some(sync_data) = sync_data {
            json!({"command": "sync_get", 
                   "uname": sync_data.uname.as_str(),
                   "pfp": sync_data.pfp.as_str(),
                   "status": Status::Ok as i32})
        } else {
            json!({"command": "sync_get", "status": Status::NotFound as i32})
        }
    }
}
impl Packet for SyncSetServersPacket {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if !peer.logged_in {
            return json!({"command": "sync_set_servers", "status": Status::Forbidden as i32});
        }

        diesel::delete(schema::sync_servers::table
                .filter(schema::sync_servers::user_uuid.eq(peer.user)))
                .execute(&state_lock.conn).unwrap();

        let mut idx = 0;
        for sync_server in &self.servers {
            let mut server = sync_server.clone();
            server.user_uuid = peer.user;
            server.idx = idx;
            if let Err(e) = state_lock.insert_sync_server(server) {
                println!("Warn(SyncSetServersPacket::execute) error setting sync server: {:?}", e);
                return json!({"command": "sync_get_servers", "status": Status::InternalError as i32});
            }
            idx += 1;
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
                .load::<SyncServerQuery>(&state_lock.conn);

        match servers {
            Ok(servers) => {
                let servers = servers
                                .into_iter()
                                .map(SyncServer::from)
                                .collect::<Vec<SyncServer>>();
                json!({"command": "sync_get_servers",
                       "servers": servers,
                       "status": Status::Ok as i32})
            },
            Err(e) => {
                println!("Warn(SyncGetServersPacket::execute) error getting sync servers: {:?}", e);
                json!({"command": "sync_set_servers", "status": Status::InternalError as i32})
            }
        }
    }
}


pub async fn process_command(msg: &String, state: Arc<Mutex<Shared>>, peer: &mut Peer) -> Result<(), Box<dyn Error>> {
    let response = match serde_json::from_str::<Packets>(msg) {
        Ok(packets) => {
            let mut state_lock = state.lock().await;
            packets.execute(&mut state_lock, peer)
        },
        Err(e) => {
            println!("Warn(process_command) error decoding packet '{}': {:?}", msg, e);
            json!({"command": "unknown", "status": Status::BadRequest as i32})
        }
    };

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

