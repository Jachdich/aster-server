extern crate tokio;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tokio_native_tls::{TlsStream};
use tokio_util::codec::{Framed, LinesCodec};

#[macro_use]
extern crate diesel;
use diesel::prelude::*;

use futures::SinkExt;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::io::Read;

pub mod schema;
pub mod models;
pub mod shared;
pub mod serverproperties;
pub mod sharedchannel;
pub mod message;
pub mod peer;
pub mod helper;
pub mod permissions;

use models::*;
use message::*;
use peer::Peer;
use peer::Pontoon;
use shared::Shared;
use helper::gen_uuid;

//release.major.minor
const API_VERSION_RELEASE: u8 = 0;
const API_VERSION_MAJOR: u8 = 1;
const API_VERSION_MINOR: u8 = 3;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let state = Arc::new(Mutex::new(Shared::new()));

    {
        let mut state = state.lock().await;
        state.load();
    }

    //start voice listener to connect to the voice server
    let vstate = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            if let Err(e) = listen_for_voice(&vstate).await {
                println!("Voice server error: {:?}", e);
            }
        }
    });
    
    let addr = "0.0.0.0:2345".to_string();
    
    let listener = TcpListener::bind(&addr).await?;

    let der = include_bytes!("../identity.pfx");
    let cert = native_tls::Identity::from_pkcs12(der, "").unwrap();

    let tls_acceptor = tokio_native_tls::TlsAcceptor::from(
        native_tls::TlsAcceptor::builder(cert).build().unwrap()
    );


    loop {
        let (stream, addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();

        let state = Arc::clone(&state);

        tokio::spawn(async move {
            let tls_stream = tls_acceptor.accept(stream).await.expect("Accept error");
            if let Err(e) = process(state, tls_stream, addr).await {
                println!("An error occurred: {:?}", e);
            }
        });
    }
}

async fn listen_for_voice<'a>(state: &Arc<Mutex<Shared>>) -> Result<(), Box<dyn Error>> {
    println!("Starting voice server on 0.0.0.0:5432");
    let addr = "0.0.0.0:5432";
    let listener = TcpListener::bind(&addr).await?;
    let (stream, addr) = listener.accept().await?;
    println!("Got voice server connection at {}", addr);
    let mut lines = Framed::new(stream, LinesCodec::new());

    let mut joined: Vec<i64> = Vec::new();
    
    while let Some(Ok(result)) = lines.next().await {
        let parsed = json::parse(&result);
        match parsed {
            Ok(parsed) => {
                //let mut peer = state
                if parsed["command"] == "join" {
                    joined.push(parsed["uuid"].as_i64().unwrap());
                    {
                        let state = state.lock().await;
                        for peer in &state.peers {
                            if joined.contains(&peer.uuid) {
                                peer.tx.send(MessageType::Internal(InternalMessage{ content: json::object!{command: "someone joined voice", uuid: parsed["uuid"].as_i64().unwrap() } } ));
                            }
                        }
                    }
                }
                if parsed["command"] == "leave" {
                    let mut idx = 0;
                    for peer in &joined {
                        if *peer == parsed["uuid"].as_i64().unwrap() {
                            joined.remove(idx);
                            break;
                        }
                        idx += 1;
                    }
                    {
                        let state = state.lock().await;
                        for peer in &state.peers {
                            if joined.contains(&peer.uuid) {
                                peer.tx.send(MessageType::Internal(InternalMessage { content: json::object!{command: "someone left voice", uuid: parsed["uuid"].as_i64().unwrap() } } ));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Couldnt parse \"{}\" as json! ({:?})", result, e);
            }
        }
    }
    println!("Voice server sent nothing lol");
    Ok(())
}

fn send_metadata(state_lock: &tokio::sync::MutexGuard<'_, Shared>, peer: &Peer) {
    let meta = json::array![state_lock.get_user(&peer.user).as_json()];
    state_lock.channels.get(&peer.channel).unwrap().broadcast(
        peer.addr,
        MessageType::Raw(RawMessage{content: json::object!{command: "metadata", data: meta}.dump()}),
        &state_lock);
}

fn send_online(state_lock: &tokio::sync::MutexGuard<'_, Shared>) {
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

async fn process_command(msg: &String, state: Arc<Mutex<Shared>>, peer: &mut Peer) -> Result<(), Box<dyn Error>> {
    let split = msg.split(" ");
    let argv = split.collect::<Vec<&str>>();
    let mut state_lock = state.lock().await;

    //commands that can be run when logged in or logged out
    match argv[0] {
        "/get_all_metadata" => {
            let mut meta = json::JsonValue::new_array();
            for v in &state_lock.get_users() {
                meta.push(v.as_json()).unwrap();
            }
            peer.lines.send(json::object!{command: "metadata", data: meta}.dump()).await?;
        }

        "/get_icon" => {
            peer.lines.send(json::object!{command: "get_icon", data: state_lock.properties.pfp.to_owned()}.dump()).await?;
        }
        "/get_name" => {
            peer.lines.send(json::object!{command: "get_name", data: state_lock.properties.name.to_owned()}.dump()).await?;
        }
        "/get_channels" => {
            let mut res = json::JsonValue::new_array();
            let channels = state_lock.get_channels();
            for channel in channels {
                res.push(channel.as_json()).unwrap();
            }
            
            peer.lines.send(json::object!{command: "get_channels", data: res}.dump()).await?;
        }
        "/ping" => {
            peer.lines.send(json::object!{command: "pong"}.dump()).await?;
        }
        "/leave" => {
            peer.lines.send(json::object!{command: "Goodbye"}.dump()).await?;
        }

        "/get_emoji" => {
            if argv.len() != 2 {
                peer.lines.send(json::object!{command: "get_emoji", code: -1, message: "Wrong number of arguments"}.dump()).await?;
            } else {
                if let Ok(uuid) = argv[1].parse::<i64>() {
                    let results = schema::emojis::table
                        .filter(schema::emojis::uuid.eq(uuid))
                        .limit(1)
                        .load::<Emoji>(&state_lock.conn).unwrap();
                    if results.len() < 1 {
                        peer.lines.send(json::object!{command: "get_emoji", code: -2, message: "Emoji not found"}.dump()).await?;
                    } else {
                        peer.lines.send(json::object!{command: "get_emoji", code: 0, data: results[0].as_json()}.dump()).await?;
                    }
                } else {
                    peer.lines.send(json::object!{command: "get_emoji", code: -1, message: "Argument is not an integer"}.dump()).await?;
                }
            }
        }

        _ => {}
    }

    //commands that can be run only if the user is logged out
    if !peer.logged_in {
        match argv[0] {
            "/register" => {
                //register new user with metadata
                let pfp: String;
                match std::fs::File::open("default.png") {
                    Ok(mut file) => {
                        let mut data = Vec::new();
                        file.read_to_end(&mut data).unwrap();
                        pfp = base64::encode(data);
                    }
                    Err(e) => {
                        panic!("{} Couldn't read default profile picture. Please provide default.png!", e);
                    }
                }

                let uuid = gen_uuid();
                let user = User{
                    name: json::stringify(uuid),
                    pfp: pfp,
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
            let mut res = json::JsonValue::new_array();
            for user in state_lock.online.iter() {
                res.push(user + 0).unwrap();
            }
            let final_json = json::object!{
                command: "online",
                data: res,
            };
            peer.lines.send(&final_json.dump()).await?;
        }

        "/join" => {
            if argv.len() < 2 {
                peer.lines.send("Usage: /join <[#|&]channel>").await?;
            } else {
                peer.channel(state_lock.get_channel_by_name(&argv[1].to_string()).uuid, &mut state_lock);
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
                peer.lines.send(json::object!{request: "sync_get", message: "Too few arguments", code: -1}.dump()).await?; 
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
                         peer.lines.send(json::object!{request: "sync_get", key: argv[1], message: "Invalid key", code: -1}.dump()).await?,   
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
                peer.lines.send(json::object!{request: "sync_add_server", code: -1, message: "Invalid JSON data"}.dump()).await?;
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
                peer.lines.send(json::object!{request: "sync_add_server", code: -1, message: "Invalid JSON data"}.dump()).await?;
            }
        }

        "/sync_get_servers" => {
            let servers = schema::sync_servers::table
                    .filter(schema::sync_servers::user_uuid.eq(peer.user))
                    .order(schema::sync_servers::idx.asc())
                    .load::<SyncServerQuery>(&state_lock.conn).unwrap();

            peer.lines.send(json::object!{
                    request: "sync_get_servers",
                    data: servers.iter().map(|x| SyncServer::from(x.clone()).as_json()).collect::<Vec<json::JsonValue>>(),
                    code: 0}.dump()).await?;
        }

        "/sync_get" => {
            let sync_data = state_lock.get_sync_data(&peer.user);
            if let Some(sync_data) = sync_data {
                peer.lines.send(json::object!{request: "sync_get", uname: sync_data.uname.as_str(), pfp: sync_data.pfp.as_str(), code: 0}.dump()).await?;
            } else {
                peer.lines.send(json::object!{request: "sync_get", key: argv[1], message: "User has no sync data", code: -2}.dump()).await?;
            }
        }
        _ => {
            peer.lines.send(json::object!{request: argv[0], message: "Invalid command", code: -1}.dump()).await?;
        }
    }
    Ok(())
}

async fn process_internal_command(msg: &InternalMessage, state: Arc<Mutex<Shared>>, peer: &mut Peer) -> Result<(), Box<dyn Error>> {
    println!("internal command: {:?}", msg);
    Ok(())
}

async fn process(state: Arc<Mutex<Shared>>, stream: TlsStream<TcpStream>, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let channel: i64;
    {
        let state = state.lock().await;
        channel = state.get_channel_by_name(&"general".to_string()).uuid;
    }
    let lines = Framed::new(stream, LinesCodec::new());
    let mut peer = Peer::new(state.clone(), lines, channel, addr).await?;
    {
        let mut state = state.lock().await;
        state.peers.push(Pontoon::from_peer(&peer));
    }

    peer.lines.send(json::object!{command: "API_version", rel: API_VERSION_RELEASE, maj: API_VERSION_MAJOR, min: API_VERSION_MINOR}.dump()).await?;
    //TODO handshake protocol
    
    while let Some(result) = peer.next().await {
        match result {
            Ok(Message::Broadcast(msg)) => {
                match msg {
                    MessageType::Cooked(msg) => {
                        if msg.content.len() == 0 {
                            continue;
                        }
                        if msg.content.chars().nth(0).unwrap() == '/' {
                            process_command(&msg.content, state.clone(), &mut peer).await?;
                        } else {
                            if peer.logged_in {
                                let state_lock = state.lock().await;
                                state_lock.channels.get(&peer.channel).unwrap().broadcast(
                                    addr, MessageType::Cooked(msg), &state_lock);
                            }
                        }
                    }
                    MessageType::Raw(msg) => {
                        //this doesn't make sense
                        panic!("User recv'd message '{:?}' type Raw from the client for some reason. This is a server bug", msg);
                    }
                    MessageType::Internal(msg) => {
                        //This also doesn't make any sense
                        panic!("User recv'd message '{:?}' type Internal from the client for some reason. This is a server bug", msg);
                    }
                }
            }

            Ok(Message::Received(msg)) => {
                match msg {
                    MessageType::Cooked(msg) => {
                        let mut msg_json: json::JsonValue = msg.as_json();
                        msg_json["command"] = "content".into();
                        peer.lines.send(&msg_json.dump()).await?;
                    }
                    MessageType::Raw(msg) => {
                        peer.lines.send(&msg.content).await?;
                    }
                    MessageType::Internal(msg) => {
                        process_internal_command(&msg, state.clone(), &mut peer).await?;
                    }
                }
            }

            Err(e) => { println!("Error lmao u figure it out: {}", e); }
        }
    }

    if peer.user != i64::MAX {
        let mut state = state.lock().await;
        let index = state.online.iter().position(|x| *x == peer.user).unwrap();
        state.online.remove(index);
        send_online(&state);
    }

    Ok(())
}
