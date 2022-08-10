
mod auth;
mod log_any;
mod log_in;
mod log_out;

use auth::*;
use log_any::*;
use log_in::*;
use log_out::*;

use crate::schema;
use crate::models::{User, Emoji, SyncData, SyncServer, SyncServerQuery};
use crate::helper::{gen_uuid, LockedState, JsonValue};
use crate::shared::Shared;
use crate::peer::Peer;
use crate::message::{CookedMessage, MessageType};
use crate::CONF;
use crate::helper::NO_UID;

use std::sync::Arc;
use tokio::sync::Mutex;
use std::error::Error;
use diesel::prelude::*;
use futures::SinkExt;
use std::io::Read;
use sodiumoxide::crypto::pwhash::argon2id13;
use serde_json::json;
use serde::{Deserialize, Serialize};
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
pub struct RegisterPacket { pub passwd: String, pub name: String }

#[derive(Deserialize)]
pub struct LoginPacket {
    pub passwd: String,
    pub uname: Option<String>,
    pub uuid: Option<i64>,
}

#[derive(Deserialize)]
pub struct PingPacket;

#[enum_dispatch]
#[derive(Deserialize)]
#[serde(tag = "command")]
enum Packets {
    #[serde(rename = "register")] RegisterPacket,
    #[serde(rename = "login")]    LoginPacket,
    #[serde(rename = "ping")]     PingPacket,
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
    state_lock.send_to_all(MessageType::Raw(json!({"command": "metadata", "data": meta})));
}

pub fn send_online(state_lock: &LockedState) {
    let mut res = Vec::new();
    for user in state_lock.online.iter() {
        res.push(json!(user));
    }
    let final_json = json!({
        "command": "online",
        "data": res,
    });
    state_lock.send_to_all(MessageType::Raw(final_json));
}

fn make_hash_b64(passwd: &str) -> String {
    sodiumoxide::init().expect("Fatal(hash) sodiumoxide couldn't be initialised");
    let hash = argon2id13::pwhash(
        passwd.as_bytes(),
        argon2id13::OPSLIMIT_INTERACTIVE,
        argon2id13::MEMLIMIT_INTERACTIVE
    ).expect("Fatal(hash) argon2id13::pwhash failed");
    base64::encode(&hash.0)
}

impl Packet for RegisterPacket {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if peer.logged_in {
            //registering doesn't make sense when logged in
            return json!({"command": "register", "status": Status::MethodNotAllowed as i32});
        }
    
        let uuid = gen_uuid();
        let user = User{
            name: self.name.to_owned(),
            pfp: CONF.default_pfp.to_owned(),
            uuid,
            group_uuid: 0,
        };

        state_lock.insert_user(user);
        peer.logged_in = true;
        peer.user = uuid;

        if state_lock.online.iter().any(|x| *x == peer.user) {
            println!("Error(register): user already online?");
        } else {
            state_lock.online.push(peer.user);
        }

        send_metadata(state_lock, peer);
        send_online(state_lock);

        json!({"command": "register", "status": Status::Ok as i32, "uuid": uuid})
    }
}

impl Packet for LoginPacket {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if peer.logged_in {
            //logging in doesn't make sense when already logged in
            return json!({"command": "login", "status": Status::MethodNotAllowed as i32});
        }
    
        let uuid = if let Some(uname) = &self.uname {
            if let Some(user) = state_lock.get_user_by_name(uname) { user.uuid }
            else {
                return json!({"command": "login", "status": Status::NotFound as i32});
            }
        } else if let Some(uuid) = self.uuid {
            uuid
        } else {
            //neither uname nor uuid were provided
            return json!({"command": "login", "status": Status::BadRequest as i32});
        };

        //TODO confirm password
        peer.user = uuid;
        peer.logged_in = true;
        if state_lock.online.iter().any(|x| *x == peer.user) {
            println!("Error(login): user already online?");
        } else {
            state_lock.online.push(peer.user);
        }
        send_metadata(&state_lock, peer);
        send_online(&state_lock);
        json!({"command": "login", "status": Status::Ok as i32, "uuid": uuid})
        
    }
}

impl Packet for PingPacket {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        json!({"command": "ping", "status": Status::Ok as i32})
    }
}

/*

pub fn nick(state_lock: &LockedState, peer: &mut Peer, packet: &json::JsonValue, logged: bool) -> json::JsonValue {
    if !logged {
        return json::object!{command: "nick", status: Status::Forbidden as i32};
    }

    if let Some(name) = packet["nick"].as_str() {
        let mut user = state_lock.get_user(&peer.user);
        user.name = name.to_string();
        state_lock.update_user(user);
        send_metadata(&state_lock, peer);
        json::object!{command: "nick", status: Status::Ok as i32}
    } else {
        json::object!{command: "nick", status: Status::BadRequest as i32}
    }
}

pub fn online(state_lock: &LockedState, logged: bool) -> json::JsonValue {
    if !logged {
        return json::object!{command: "online", status: Status::Forbidden as i32};
    }

    json::object!{
        command: "online",
        data: state_lock.online.clone(),
        status: Status::Ok as i32,
    }
}

pub fn content() -> json::JsonValue {
    if !logged {
        return json::object!{command: "content", status: Status::Forbidden as i32}
    }
    if let Some(packet["content"].is_str() && packet["channel_uuid"].is_number() {
        let msg = CookedMessage {
            uuid: gen_uuid(),
            content: packet["content"].as_str().unwrap().to_owned(),
            author_uuid: peer.user,
            channel_uuid,
            date: chrono::offset::Utc::now().timestamp() as i32,
        };
        state_lock.send_to_all(MessageType::Cooked(msg));
        json::object!{command: "content", status: Status::Ok as i32}
    } else {
        json::object!{command: "content", status: Status::BadRequest as i32}
    }
}*/

pub async fn process_command(msg: &String, state: Arc<Mutex<Shared>>, peer: &mut Peer) -> Result<(), Box<dyn Error>> {
    let val: Packets = serde_json::from_str(msg).unwrap();
    let mut state_lock = state.lock().await;
    let response = val.execute(&mut state_lock, peer);
    peer.lines.send(response.to_string() + "\n").await?;
/*    let packet_json = json::parse(msg);
    

    let response = if let Ok(packet) = packet {

        let mut state_lock = state.lock().await;
        let logged = peer.logged_in;
        
        
        match packet["command"].as_str() {
            //log any
            Some("get_all_metadata") => get_all_metadata(&state_lock),
            Some("get_icon")         => get_icon(&state_lock),
            Some("get_name")         => get_name(&state_lock),
            Some("get_channels")     => get_channels(&state_lock),
            Some("ping")             => json::object!{command: "pong"}, //TODO should these be functions?
            Some("leave")            => json::object!{command: "Goodbye"}, //TODO actually leave?
            Some("get_emoji")        => get_emoji(&state_lock, &packet),
            Some("list_emoji")       => list_emoji(&state_lock),

            //log out
            Some("register")        => register(&mut state_lock, peer, &packet, logged),
            Some("login")           => login(&mut state_lock, peer, &packet, logged),
            //log in
            Some("nick")            => nick(state_lock, peer, logged),
            Some("online")          => online(state_lock, logged),
            Some("content")         => content(),
            None                    => json::object!{command: "unknown", code: Status::BadRequest as i32},
            _                       => json::object!{command: packet["command"].as_str().unwrap(), code: Status::BadRequest as i32},
        }*/
/*
        //commands that can be run only if the user is logged in
        match argv[0] {
            "/history" => {
                let a = argv[1].parse::<i64>().unwrap();
                //let mut b = argv[2].parse::<usize>().unwrap();
                //if a > history.len() { a = history.len(); }
                //if b > history.len() { b = history.len(); }
                let mut history = schema::messages::table.filter(schema::messages::channel_uuid.eq(peer.channel)).order(schema::messages::rowid.desc()).limit(a).load::<CookedMessage>(&state_lock.conn).unwrap();
                history.reverse();
                let mut res = json::JsonValue::new_array();

                for msg in history.iter() {
                    //peer.lines.send(msg).await;
                    res.push(msg.as_json()).unwrap();
                }
                let json_obj = json::object!{command: "history", data: res};
                peer.lines.send(&json_obj.dump()).await?;
            }

            "/pfp" => {
                if argv.len() < 2 {
                    peer.lines.send("Usage: /pfp <base64 encoded PNG file>").await?;
                    return Ok(());
                }
                let mut user = state_lock.get_user(&peer.user);
                user.pfp = argv[1].to_string();
                state_lock.update_user(user);

                send_metadata(&state_lock, peer);

            }
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

            "/sync_set" => {
                if argv.len() < 3 {
                    peer.lines.send(json::object!{command: "sync_set", message: "Too few arguments", code: -1}.dump()).await?; 
                } else {
                    let val = argv[2..].join(" ");
                    let mut sync_data = match state_lock.get_sync_data(&peer.user) {
                        Some(data) => data,
                        None => {
                            let data = SyncData::new(peer.user);
                            state_lock.insert_sync_data(&data);
                            data
                        }
                    };
                    
                    match argv[1] {
                        "uname" => sync_data.uname = val,
                        "pfp" => sync_data.pfp = val,
                        _ => //TODO overkill?
                             peer.lines.send(json::object!{command: "sync_get", key: argv[1], message: "Invalid key", code: -1}.dump()).await?,   
                    }

                    state_lock.update_sync_data(sync_data);
                }
            }

            "/sync_add_server" => {
                let json_data = json::parse(&argv[1..].join(" "));
                if let Ok(json_data) = json_data {
                    let last_server = schema::sync_servers::table
                            .filter(schema::sync_servers::user_uuid.eq(peer.user))
                            .order(schema::sync_servers::idx.desc())
                            .limit(1)
                            .load::<SyncServerQuery>(&state_lock.conn).unwrap();
                    let idx = if last_server.len() > 0 {
                        last_server[0].idx + 1
                    } else {
                        0
                    };
                    let server = SyncServer::from_json(&json_data, peer.user, idx);
                    state_lock.insert_sync_server(server);
                } else {
                    peer.lines.send(json::object!{command: "sync_add_server", code: -1, message: "Invalid JSON data"}.dump()).await?;
                }
            }

            "/sync_set_servers" => {
                let json_data = json::parse(&argv[1..].join(" "));
                if let Ok(json_data) = json_data {
                    diesel::delete(schema::sync_servers::table
                            .filter(schema::sync_servers::user_uuid.eq(peer.user)))
                            .execute(&state_lock.conn).unwrap();

                    let mut idx = 0;
                    for sync_json in json_data["data"].members() {
                        let server = SyncServer::from_json(&sync_json, peer.user, idx);
                        state_lock.insert_sync_server(server);
                        idx += 1;
                    }
                } else {
                    peer.lines.send(json::object!{command: "sync_add_server", code: -1, message: "Invalid JSON data"}.dump()).await?;
                }
            }

            "/sync_get_servers" => {
                let servers = schema::sync_servers::table
                        .filter(schema::sync_servers::user_uuid.eq(peer.user))
                        .order(schema::sync_servers::idx.asc())
                        .load::<SyncServerQuery>(&state_lock.conn).unwrap();

                peer.lines.send(json::object!{
                        command: "sync_get_servers",
                        data: servers.iter().map(|x| SyncServer::from(x.clone()).as_json()).collect::<Vec<json::JsonValue>>(),
                        code: 0}.dump()).await?;
            }

            "/sync_get" => {
                let sync_data = state_lock.get_sync_data(&peer.user);
                if let Some(sync_data) = sync_data {
                    peer.lines.send(json::object!{command: "sync_get", uname: sync_data.uname.as_str(), pfp: sync_data.pfp.as_str(), code: 0}.dump()).await?;
                } else {
                    peer.lines.send(json::object!{command: "sync_get", message: "User has no sync data", code: -2}.dump()).await?;
                }
            }
            _ => ()
        }*/
    /*} else {
        json::object!{command: "unknown", status: Status::BadRequest as i32}
    };*/
    Ok(())
}

