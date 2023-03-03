// #![deny(warnings)]

extern crate tokio;
extern crate lazy_static;
extern crate diesel;

use serde::Deserialize;
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, ReadBuf};
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
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

pub mod commands;
pub mod helper;
pub mod message;
pub mod models;
pub mod peer;
pub mod permissions;
pub mod schema;
pub mod shared;

use crate::commands::send_online;
use crate::helper::NO_UID;
use peer::Peer;
use peer::Pontoon;
use shared::Shared;

const API_VERSION_RELEASE: u8 = 0;
const API_VERSION_MAJOR: u8 = 3;
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

struct ShittyStream {
    c: char,
    s: TlsStream<TcpStream>,
}

impl AsyncRead for ShittyStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>
    ) -> Poll<Result<(), std::io::Error>> {
        if self.c != '\0' {
            buf.put_slice(&[self.c as u8]);
            self.c = '\0';
            Poll::Ready(Ok(()))
        } else {
            Pin::new(&mut self.s).poll_read(cx, buf)
        }
    }
}

impl AsyncWrite for ShittyStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8]
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.s).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.s).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.s).poll_shutdown(cx)
    }
}

async fn process(
    state: Arc<Mutex<Shared>>,
    mut stream: TlsStream<TcpStream>,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {

    use tokio_tungstenite::tungstenite::Message;

    let mut peer = Peer::new(addr).await?;
    {
        let mut state = state.lock().await;
        state.peers.push(Pontoon::from_peer(&peer));
    }
    
    peer.tx.send(json!({"command": "API_version", "rel": API_VERSION_RELEASE, "maj": API_VERSION_MAJOR, "min": API_VERSION_MINOR, "status": 200}))?;
    //TODO handshake protocol

    //identification of whether a raw socket (json protocol) or websocket has connected
    let mut buf = vec![0; 1];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(()) // must have disconnected or something
    }
    let first_char = char::from_u32(buf[0] as u32).unwrap();

    // hang on hang on I can explain
    // so basically i have to read the first character of the stream
    // to deterime whether it is a websocket connection or a raw socket connection
    // however, TlsStream doesn't implement `.peek()`. Hmhp. So I had to do this
    // attrosity: ShittyStream basically just wraps the stream, implementing
    // AsyncRead and AsyncWrite, but the very first character that gets read
    // is the one that we read from the original stream in the first place.
    // This is a cry for help, please I don't know how to go on
    let stream_with_first_char = ShittyStream { c: first_char, s: stream };
    if first_char == '{' {
        //JSON shit
        let mut lines = Framed::new(stream_with_first_char, LinesCodec::new());

        loop {
            tokio::select! {
                result = lines.next() => match result {
                    Some(Ok(msg)) =>
                        commands::process_command(&msg, state.clone(), &mut peer).await?,
                    Some(Err(e)) => println!("Error receiving data: {}", e),
                    None => break,
                },

                Some(msg) = peer.rx.recv() => lines.send(msg.to_string()).await?,
            }
        }
    } else {
        //?? assume it's websocket
        
        let mut lines = tokio_tungstenite::accept_async(stream_with_first_char).await?;
        loop {
            tokio::select! {
                result = lines.next() => match result {
                    Some(Ok(msg)) => match msg {
                            Message::Text(msg) =>
                                commands::process_command(&msg, state.clone(), &mut peer).await?,
                            _ => (),
                        }
                    Some(Err(e)) => println!("Error receiving data: {}", e),
                    None => break,
                },

                Some(msg) = peer.rx.recv() => lines.send(Message::Text(msg.to_string())).await?,
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
