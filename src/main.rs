#![deny(warnings)]

extern crate tokio;
extern crate lazy_static;
extern crate diesel;

use serde::Deserialize;
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_native_tls::TlsStream;
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};
use base64::{Engine as _, engine::general_purpose};
use lazy_static::lazy_static;

use futures::SinkExt;
use std::error::Error;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::Arc;

pub mod commands;
pub mod helper;
pub mod message;
pub mod models;
pub mod peer;
pub mod permissions;
pub mod schema;
pub mod shared;

use crate::commands::send_online;
use crate::helper::JsonValue;
use crate::helper::NO_UID;
use message::*;
use peer::Peer;
use peer::Pontoon;
use shared::Shared;

const API_VERSION_RELEASE: u8 = 0;
const API_VERSION_MAJOR: u8 = 2;
const API_VERSION_MINOR: u8 = 0;

#[derive(Deserialize)]
pub struct Config {
    pub addr: String,
    pub port: u16,
    pub voice_port: u16,
    pub default_pfp: String,
    pub name: String,
    pub icon: String,
    pub database_file: String,
}

fn read_b64(fname: &str) -> Option<String> {
    let mut file = std::fs::File::open(fname).ok()?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).ok()?;
    Some(general_purpose::STANDARD.encode(data))
}

lazy_static! {
    pub static ref CONF: Config = {
        let mut file = std::fs::File::open("config.json").expect("Couldn't find config.json!");
        let mut data = String::new();
        file.read_to_string(&mut data)
            .expect("Couldn't read config.json!");
        let res: Result<Config, serde_json::Error> = serde_json::from_str(&data);
        match res {
            Ok(mut cfg) => {
                let default_pfp: String = read_b64(&cfg.default_pfp).expect(&format!(
                    "Default profile picture file '{}' not found!",
                    cfg.default_pfp
                ));
                let icon: String =
                    read_b64(&cfg.icon).expect(&format!("Icon file '{}' not found!", cfg.icon));

                cfg.icon = icon;
                cfg.default_pfp = default_pfp;
                cfg
            }
            Err(e) => {
                panic!("Failed to load config: {}", e);
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
    if false {
        start_voice_server(Arc::clone(&state));
    }
    let addr = format!("{}:{}", &CONF.addr, CONF.port);

    let listener = TcpListener::bind(&addr).await?;
    println!("Listening on {}", &addr);

    let der = include_bytes!("../identity.pfx");
    let cert = native_tls::Identity::from_pkcs12(der, "").unwrap();

    let tls_acceptor = tokio_native_tls::TlsAcceptor::from(
        native_tls::TlsAcceptor::builder(cert).build().unwrap(),
    );

    loop {
        let (stream, addr) = listener.accept().await?;
        println!("Got connection from {}", &addr);
        let tls_acceptor = tls_acceptor.clone();

        let state = Arc::clone(&state);

        tokio::spawn(async move {
            let tls_stream = tls_acceptor.accept(stream).await.expect("Accept error");
            if let Err(e) = process(state, tls_stream, addr).await {
                println!("An error occurred: {:?}", e);
            }
            println!("Lost connection from {}", &addr);
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
    let addr = format!("{}:{}", &CONF.addr, CONF.voice_port);
    println!("Starting voice server on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    let (stream, addr) = listener.accept().await?;
    println!("Got voice server connection at {}", addr);
    let mut lines = Framed::new(stream, LinesCodec::new());

    let mut joined: Vec<i64> = Vec::new();

    while let Some(Ok(result)) = lines.next().await {
        let parsed: Result<JsonValue, serde_json::Error> = serde_json::from_str(&result);
        match parsed {
            Ok(parsed) => {
                //let mut peer = state
                if parsed["command"] == "join" {
                    joined.push(parsed["uuid"].as_i64().unwrap());
                    {
                        let state = state.lock().await;
                        for peer in &state.peers {
                            if joined.contains(&peer.uuid) {
                                peer.tx.send(MessageType::Internal(json!({"command": "someone joined voice", "uuid": parsed["uuid"].as_i64().unwrap() })))?;
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
                                peer.tx.send(MessageType::Internal(json!({"command": "someone left voice", "uuid": parsed["uuid"].as_i64().unwrap() })))?;
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

async fn process_internal_command(
    msg: &JsonValue,
    _state: Arc<Mutex<Shared>>,
    _peer: &mut Peer,
) -> Result<(), Box<dyn Error>> {
    println!("internal command: {:?}", msg);
    Ok(())
}

async fn process(
    state: Arc<Mutex<Shared>>,
    stream: TlsStream<TcpStream>,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let lines = Framed::new(stream, LinesCodec::new());
    let mut peer = Peer::new(lines, addr).await?;
    {
        let mut state = state.lock().await;
        state.peers.push(Pontoon::from_peer(&peer));
    }

    peer.lines.send(json!({"command": "API_version", "rel": API_VERSION_RELEASE, "maj": API_VERSION_MAJOR, "min": API_VERSION_MINOR, "status": 200}).to_string()).await?;
    //TODO handshake protocol

    while let Some(result) = peer.next().await {
        match result {
            Ok(Message::Broadcast(msg)) => {
                commands::process_command(&msg, state.clone(), &mut peer).await?;
            }

            Ok(Message::Received(msg)) => match msg {
                MessageType::Cooked(msg) => {
                    let mut msg_json = serde_json::to_value(&msg)?;
                    msg_json["command"] = "content".into();
                    msg_json["status"] = 200.into();
                    peer.lines.send(&msg_json.to_string()).await?;
                }
                MessageType::Raw(msg) => peer.lines.send(msg.to_string()).await?,
                MessageType::Internal(msg) => {
                    process_internal_command(&msg, state.clone(), &mut peer).await?
                }
            },

            Err(e) => {
                println!("Error lmao u figure it out: {}", e);
            }
        }
    }

    let mut state = state.lock().await;
    if peer.user != NO_UID {
        let count = *state.online.get(&peer.user).unwrap_or(&0);
        if count > 0 {
            state.online.insert(peer.user, count - 1);
        }
        if count == 1 {
            send_online(&state);
        }
    }
    if let Some(index) = state.peers.iter().position(|x| x.addr == peer.addr) {
        state.peers.remove(index);
    }

    Ok(())
}
