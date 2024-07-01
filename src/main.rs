// #![deny(warnings)]

extern crate diesel;
extern crate lazy_static;
extern crate tokio;

use base64::{engine::general_purpose, Engine as _};
use lazy_static::lazy_static;
use serde::Deserialize;
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
// use tokio_native_tls::TlsStream;
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use futures::SinkExt;
use std::error::Error;
use std::io::Read;
use std::pin::Pin;
use std::sync::Arc;
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
use peer::Peer;
use shared::Shared;

const API_VERSION: [u8; 3] = [0, 1, 0]; // major, minor, patch

//DEBUG
type SocketStream = TcpStream;

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
                let default_pfp: String = read_b64(&cfg.default_pfp).unwrap_or_else(|| {
                    panic!(
                        "Default profile picture file '{}' not found!",
                        cfg.default_pfp
                    )
                });
                let icon: String = read_b64(&cfg.icon)
                    .unwrap_or_else(|| panic!("Icon file '{}' not found!", cfg.icon));

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
    let mut builder = env_logger::Builder::from_default_env();
    builder.filter_level(log::LevelFilter::Info).init();

    let state = Arc::new(Mutex::new(Shared::new()));

    {
        let mut state = state.lock().await;
        state.load();
    }

    let addr = format!("{}:{}", &CONF.addr, CONF.port);

    let listener = TcpListener::bind(&addr).await?;
    log::info!("Listening on {}", &addr);

    // TLS stuff, disable for testing
    /*
    let der = include_bytes!("../identity.pfx");
    let cert = native_tls::Identity::from_pkcs12(der, "").unwrap();

    let tls_acceptor = tokio_native_tls::TlsAcceptor::from(
        native_tls::TlsAcceptor::builder(cert).build().unwrap(),
    );*/

    loop {
        let (stream, addr) = listener.accept().await?;
        log::info!("Got connection from {}", &addr);
        // let tls_acceptor = tls_acceptor.clone();

        let state = Arc::clone(&state);

        tokio::spawn(async move {
            // let tls_stream = tls_acceptor.accept(stream).await.expect("Accept error");
            let mut peer = Peer::new(addr);
            if let Err(e) = process(Arc::clone(&state), /*tls_*/ stream, &mut peer).await {
                log::error!("An error occurred in the connection:\n{:?}", e);
            }
            log::info!("Lost connection from {}", &addr);

            let mut state = state.lock().await;
            if let Some(uuid) = peer.uuid {
                let count = *state.online.get(&uuid).unwrap_or(&0);
                if count > 0 {
                    state.online.insert(uuid, count - 1);
                }
                if count == 1 {
                    send_online(&state);
                }
            }
            if let Some(index) = state.peers.iter().position(|x| x.1 == peer.addr) {
                state.peers.remove(index);
            }
        });
    }
}

struct ShittyStream {
    c: char,
    // s: TlsStream<TcpStream>,
    s: SocketStream,
}

impl AsyncRead for ShittyStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
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
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.s).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.s).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.s).poll_shutdown(cx)
    }
}

async fn process(
    state: Arc<Mutex<Shared>>,
    mut stream: SocketStream,
    peer: &mut Peer,
) -> Result<(), Box<dyn Error>> {
    use tokio_tungstenite::tungstenite::Message;

    {
        let mut state = state.lock().await;
        state.peers.push((peer.tx.clone(), peer.addr));
    }
    peer.tx
        .send(json!({"command": "API_version", "version": API_VERSION, "status": 200}))?;
    //TODO handshake protocol

    //identification of whether a raw socket (json protocol) or websocket has connected
    let mut buf = vec![0; 1];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(()); // must have disconnected or something
    }
    let first_char = char::from_u32(buf[0] as u32).unwrap();

    // hang on hang on I can explain
    // so basically i have to read the first character of the stream
    // to deterime whether it is a websocket connection or a raw socket connection
    // however, TlsStream doesn't implement `.peek()`. Hmph. So I had to do this
    // attrosity: ShittyStream basically just wraps the stream, implementing
    // AsyncRead and AsyncWrite, but the very first character that gets read
    // is the one that we read from the original stream in the first place.
    // This is a cry for help, please I don't know how to go on
    let stream_with_first_char = ShittyStream {
        c: first_char,
        s: stream,
    };

    if first_char == '{' {
        //JSON shit
        let mut lines = Framed::new(stream_with_first_char, LinesCodec::new());

        loop {
            tokio::select! {
                result = lines.next() => match result {
                    Some(Ok(msg)) =>
                        commands::process_command(&msg, &mut state.lock().await, peer)?,
                    Some(Err(e)) => log::error!("Error receiving data: {}", e),
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
                                commands::process_command(&msg, &mut state.lock().await, peer)?,
                            _ => log::warn!("Got non-text websocket message: {:?}", msg), // TODO handle this properly
                        }
                    Some(Err(e)) => log::error!("Error receiving data: {}", e),
                    None => break,
                },

                Some(msg) = peer.rx.recv() => lines.send(Message::Text(msg.to_string())).await?,
            }
        }
    }

    Ok(())
}
