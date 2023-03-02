// #![deny(warnings)]

extern crate tokio;
extern crate lazy_static;
extern crate diesel;

use serde::Deserialize;
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::io::AsyncReadExt;
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
                let default_pfp: String = read_b64(&cfg.default_pfp).unwrap_or_else(|| panic!(
                    "Default profile picture file '{}' not found!",
                    cfg.default_pfp
                ));
                let icon: String =
                    read_b64(&cfg.icon).unwrap_or_else(|| panic!("Icon file '{}' not found!", cfg.icon));

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

async fn process(
    state: Arc<Mutex<Shared>>,
    mut stream: TlsStream<TcpStream>,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    // let ws_stream = tokio_tungstenite::accept_async(stream).await?;
       
    let lines = Framed::new(stream, LinesCodec::new());
    let mut peer = Peer::new(lines, addr).await?;
    {
        let mut state = state.lock().await;
        state.peers.push(Pontoon::from_peer(&peer));
    }

    peer.lines.send(json!({"command": "API_version", "rel": API_VERSION_RELEASE, "maj": API_VERSION_MAJOR, "min": API_VERSION_MINOR, "status": 200}).to_string()).await?;
    //TODO handshake protocol

    loop {
        tokio::select! {
            result = peer.lines.next() => match result {
                Some(Ok(msg)) =>
                    commands::process_command(&msg, state.clone(), &mut peer).await?,
                Some(Err(e)) => println!("Error receiving data: {}", e),
                None => break,
            },

            Some(msg) = peer.rx.recv() => peer.lines.send(msg.to_string()).await?,
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
