extern crate tokio;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate diesel;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tokio_native_tls::{TlsStream};
use tokio_util::codec::{Framed, LinesCodec};

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
pub mod commands;

use message::*;
use peer::Peer;
use peer::Pontoon;
use shared::Shared;
use crate::commands::send_online;

//release.major.minor
const API_VERSION_RELEASE: u8 = 0;
const API_VERSION_MAJOR: u8 = 2;
const API_VERSION_MINOR: u8 = 0;

pub struct Config {
    pub addr: String,
    pub port: u16,
    pub default_pfp: String,
    pub name: String,
    pub icon: String,
    pub database_file: String,
}

//used in conf file loading, read a file to a base64 string and panic with supplied message on any error
fn read_b64_panic(fname: &str, msg: &str) -> String {
    if let Ok(mut file) = std::fs::File::open(fname) {
        let mut data = Vec::new();
        file.read_to_end(&mut data).expect(msg);
         base64::encode(data)
    } else {
        panic!("{}", msg);
    }
}

lazy_static! {
    pub static ref CONF: Config = {
        match std::fs::File::open("config.json") {
            Ok(mut file) => {
                let mut data = String::new();
                file.read_to_string(&mut data).expect("Couldn't read config.json!");
                let json_value = json::parse(&data).expect("config.json is not valid json!");

                let default_pfp_path = json_value["default_pfp"].as_str().expect("'default_pfp' value must be string");
                let icon_path = json_value["icon"].as_str().expect("'icon' value must be string");
                
                let default_pfp: String = read_b64_panic(
                        default_pfp_path,
                        &format!("Default profile picture file '{}' not found!", default_pfp_path));
                let icon: String = read_b64_panic(
                        icon_path,
                        &format!("Icon file '{}' not found!", icon_path));
                
                Config {
                    addr: json_value["address"].as_str().expect("'address' value must be string").to_owned(),
                    port: json_value["port"].as_u16().expect("'port' value must be 16 bit unsigned integer"),
                    name: json_value["name"].as_str().expect("'name' value must be string").to_owned(),
                    database_file: json_value["database_file"].as_str().expect("'database_file' value must be string").to_owned(),
                    default_pfp, icon,
                }
            }
            Err(_) => {
                panic!("Couldn't find config.json!");
            }
        }
    };
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let state = Arc::new(Mutex::new(Shared::new()));

    {
        let mut state = state.lock().await;
        state.load();
    }

    //start_voice_server(Arc::clone(&state));
    
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

fn start_voice_server(state: Arc<Mutex<Shared>>) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = listen_for_voice(&state).await {
                println!("Voice server error: {:?}", e);
            }
        }
    });
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

async fn process_internal_command(msg: &InternalMessage, state: Arc<Mutex<Shared>>, peer: &mut Peer) -> Result<(), Box<dyn Error>> {
    println!("internal command: {:?}", msg);
    Ok(())
}

async fn process(state: Arc<Mutex<Shared>>, stream: TlsStream<TcpStream>, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let lines = Framed::new(stream, LinesCodec::new());
    let mut peer = Peer::new(state.clone(), lines, addr).await?;
    {
        let mut state = state.lock().await;
        state.peers.push(Pontoon::from_peer(&peer));
    }

    peer.lines.send(json::object!{command: "API_version", rel: API_VERSION_RELEASE, maj: API_VERSION_MAJOR, min: API_VERSION_MINOR}.dump()).await?;
    //TODO handshake protocol
    
    while let Some(result) = peer.next().await {
        match result {
            Ok(Message::Broadcast(msg)) => {
                if let MessageType::Cooked(msg) = msg {
                    if msg.content.len() == 0 {
                        continue;
                    }
                    if msg.content.chars().nth(0).unwrap() == '/' {
                        commands::process_command(&msg.content, state.clone(), &mut peer).await?;
                    } else {
                        if peer.logged_in {
                            let state_lock = state.lock().await;
                            state_lock.channels.get(&peer.channel).unwrap().broadcast(
                                addr, MessageType::Cooked(msg), &state_lock);
                        }
                    }
                } else {
                    panic!("User recv'd message '{:?}' from the client for some reason. This is a server bug", msg);
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
