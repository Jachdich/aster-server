extern crate tokio;
extern crate ctrlc;

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

use models::User;
use message::*;
use peer::Peer;
use shared::Shared;
use helper::gen_uuid;

use permissions::Perm;

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

async fn listen_for_voice(state: &Arc<Mutex<Shared>>) -> Result<(), Box<dyn Error>> {
    println!("Starting voice server on 0.0.0.0:5432");
    let addr = "0.0.0.0:5432";
    let listener = TcpListener::bind(&addr).await?;
    let (stream, addr) = listener.accept().await?;
    println!("Got voice server connection at {}", addr);
    let mut lines = Framed::new(stream, LinesCodec::new());
    
    while let Some(Ok(result)) = lines.next().await {
        let parsed = json::parse(&result);
        match parsed {
            Ok(parsed) => {
                if parsed["command"] == "join" {
                }
                if parsed["command"] = "leave" {
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
                res.push(channel.name).unwrap();
            }
            
            peer.lines.send(json::object!{command: "get_channels", data: res}.dump()).await?;
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

                if let Some(index) = state_lock.online.iter().position(|x| *x == peer.user) {
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
                peer.lines.send("Usage: /join <[#][&]channel>").await?;
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
            let json_obj = json::object!{history: res};
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

        "/leave" => {
            ()
        }

        "/delete" => {
            let uuid = argv[1].parse::<i64>().unwrap();
            diesel::delete(schema::users::table.filter(schema::users::uuid.eq(uuid))).execute(&state_lock.conn).unwrap();
            diesel::delete(schema::messages::table.filter(
                schema::messages::author_uuid.eq(uuid))).execute(&state_lock.conn).unwrap();
        }
        _ => ()
    }
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
                    MessageType::Raw(_msg) => {
                        //this doesn't make sense
                    }
                }
            }

            Ok(Message::Received(msg)) => {
                match msg {
                    MessageType::Cooked(msg) => {
                        peer.lines.send(&msg.as_json().dump()).await?;
                    }
                    MessageType::Raw(msg) => {
                        peer.lines.send(&msg.content).await?;
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
