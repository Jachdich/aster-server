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
use crate::helper::{gen_uuid, LockedState};
use crate::shared::Shared;
use crate::peer::Peer;
use crate::message::{RawMessage, CookedMessage, MessageType};

use std::sync::Arc;
use tokio::sync::Mutex;
use std::error::Error;
use diesel::prelude::*;
use futures::SinkExt;
use std::io::Read;

pub enum Status {
    Ok = 200,
    BadRequest = 400,
    NotFound = 404,
    InternalError = 500,
    Unauthorised = 401,
    Forbidden = 403,
}

fn send_metadata(state_lock: &LockedState, peer: &Peer) {
    let meta = json::array![state_lock.get_user(&peer.user).as_json()];
    state_lock.channels.get(&peer.channel).unwrap().broadcast(
        peer.addr,
        MessageType::Raw(RawMessage{content: json::object!{command: "metadata", data: meta}.dump()}),
        &state_lock);
}

pub fn send_online(state_lock: &LockedState) {
    let mut res = json::JsonValue::new_array();
    for user in state_lock.online.iter() {
        res.push(user + 0).unwrap();
    }
    let final_json = json::object!{
        command: "online",
        data: res,
    };
    state_lock.broadcast_to_all(MessageType::Raw(RawMessage{content: final_json.dump()}), state_lock);
}
/*
pub fn register() -> json::JsonValue {
    let uuid = gen_uuid();
    let user = User{
        name: json::stringify(uuid),
        pfp: CONF.default_pfp.to_owned(),
        uuid: uuid,
        group_uuid: 0,
    };

    state_lock.insert_user(user);
    peer.lines.send(json::object!{"command": "set", "key": "self_uuid", "value": uuid}.dump()).await?;
    peer.logged_in = true;
    peer.user = uuid;

    if let Some(_) = state_lock.online.iter().position(|x| *x == peer.user) {
        println!("Error: user already online???");
    } else {
        state_lock.online.push(peer.user);
    }

    send_metadata(&state_lock, peer);
    send_online(&state_lock);
}

*/
pub async fn process_command(msg: &String, state: Arc<Mutex<Shared>>, peer: &mut Peer) -> Result<(), Box<dyn Error>> {
    let packet = json::parse(msg);

    let response = if let Ok(packet) = packet {

        let mut state_lock = state.lock().await;
        let logged = peer.logged_in;
        
        
        match packet["command"].as_str() {
            //log in/out
            Some("/get_all_metadata") => get_all_metadata(&state_lock),
            Some("/get_icon")         => get_icon(&state_lock),
            Some("/get_name")         => get_name(&state_lock),
            Some("/get_channels")     => get_channels(&state_lock),
            Some("/ping")             => json::object!{command: "pong"}, //TODO should these be functions?
            Some("/leave")            => json::object!{command: "Goodbye"}, //TODO actually leave?
            Some("/get_emoji")        => get_emoji(&state_lock, &packet),
            Some("/list_emoji")       => list_emoji(&state_lock),

            //log out

            //log in
            None                    => json::object!{command: "unknown", code: Status::BadRequest as i32},
            _                       => json::object!{command: packet["command"].as_str().unwrap(), code: Status::BadRequest as i32},
        }
/*
        //commands that can be run only if the user is logged out
        if !peer.logged_in {
            match argv[0] {
                "/register" => {
                    
                }

                "/login" => {
                    //log in an existing user
                    let uuid = argv[1].parse::<i64>().unwrap();
                    if argv.len() <= 2 {
                        peer.lines.send(json::object!{"warning": "Logging in without password is deprecated and WILL BE REMOVED SOON. Please update your client"}.dump()).await?;
                    } else {
                        let password = argv[2];
                        let hashed_password = "";
                        //if hashed_password == state_lock.get_password(uuid);
                    }
                    peer.lines.send(json::object!{"command": "set", "key": "self_uuid", "value": uuid}.dump()).await?;
                    peer.user = uuid;
                    peer.logged_in = true;

                    state_lock.online.push(peer.user);
                    send_metadata(&state_lock, peer);
                    send_online(&state_lock);
                }

                "/login_username" => {
                    if argv.len() <= 2 {
                        peer.lines.send(json::object!{"warning": "Logging in without password is deprecated and WILL BE REMOVED SOON. Please update your client"}.dump()).await?;
                    } else {
                        let password = argv[2];
                        let hashed_password = "";
                        //if hashed_password == state_lock.get_password(uuid);
                    }
                    let uuid = schema::users::table.filter(schema::users::name.eq(argv[1])).limit(1).load::<User>(&state_lock.conn).unwrap()[0].uuid;
                    peer.lines.send(json::object!{"command": "set", "key": "self_uuid", "value": uuid}.dump()).await?;
                    peer.user = uuid;
                    peer.logged_in = true;

                    state_lock.online.push(peer.user);
                    send_metadata(&state_lock, peer);
                    send_online(&state_lock);
                }

                _ => {}
            }
            return Ok(());
        }

        //commands that can be run only if the user is logged in
        match argv[0] {
            "/nick" => {
                if argv.len() < 2 {
                    peer.lines.send("Usage: /nick <nickname>").await?;
                } else {
                    //let index = state_lock.online.iter().position(|x| *x == peer.user.name).unwrap();
                    //state_lock.online.remove(index);
                    let mut user = state_lock.get_user(&peer.user);
                    user.name = argv[1].to_string();
                    state_lock.update_user(user);
                    send_metadata(&state_lock, peer);
                    //state_lock.online.push(peer.user.name.clone());
                }
            }
            
            "/online" => {
                let final_json = json::object!{
                    command: "online",
                    data: state_lock.online.clone(),
                };
                peer.lines.send(&final_json.dump()).await?;
            }

            "/join" => {
                if argv.len() < 2 {
                    peer.lines.send("Usage: /join <[#|&]channel>").await?;
                } else {
                    let status: Status;
                    if let Ok(channel) = state_lock.get_channel_by_name(&argv[1].to_string()) {
                        peer.channel(channel.uuid, &mut state_lock);
                        status = Status::Ok;
                    } else {
                        status = Status::NotFound;
                    }
                    
                    peer.lines.send(json::object!{command: "join", status: status as i32}.dump()).await?;
               }
            }

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
    } else {
        json::object!{command: "unknown", status: Status::BadRequest as i32}
    };
    Ok(())
}

