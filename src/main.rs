// #![deny(warnings)]

extern crate lazy_static;
extern crate tokio;

use base64::{engine::general_purpose, Engine as _};
use commands::Response;
use helper::{gen_uuid, Uuid};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::upgrade::Upgraded;
use hyper::Request;
use hyper_tungstenite::WebSocketStream;
use hyper_util::rt::TokioIo;
use lazy_static::lazy_static;
use models::{Group, User};
use permissions::{Perm, Permissions};
use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use futures::SinkExt;
use std::env;
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
pub mod shared;

use crate::commands::send_online;
use peer::Peer;
use shared::Shared;

const API_VERSION: [u8; 3] = [1, 0, 0]; // major, minor, patch

#[cfg(not(feature = "notls"))]
use std::fs::File;
#[cfg(not(feature = "notls"))]
use tokio_native_tls::TlsStream;

#[cfg(feature = "notls")]
type SocketStream = TcpStream;

#[cfg(not(feature = "notls"))]
type SocketStream = TlsStream<TcpStream>;

#[derive(Deserialize)]
pub struct Config {
    pub addr: String,
    pub port: u16,
    pub voice_port: u16,
    pub default_pfp: String,
    pub name: String,
    pub icon: String,
    pub database_file: String,
    pub certificate_chain: String,
    pub private_key: String,
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

    let args: Vec<_> = env::args().collect();
    let use_scratch_db = args.len() > 1 && args[1] == "--scratch-db";

    let sqlitedb = if use_scratch_db {
        rusqlite::Connection::open_in_memory().unwrap()
    } else {
        rusqlite::Connection::open(&CONF.database_file).unwrap_or_else(|_| {
            panic!(
                "Fatal(Shared::new) connecting to the database file {}",
                &CONF.database_file
            )
        })
    };
    let shared = Shared::new(sqlitedb);
    shared.init_db();

    // DEBUG lol
    if args.len() >= 5 {
        if args[2] == "--admin-user" {
            let username = &args[3];
            let password = &args[4];
            let group = Group {
                uuid: gen_uuid(),
                permissions: Permissions {
                    modify_channels: Perm::Allow,
                    modify_icon_name: Perm::Allow,
                    modify_groups: Perm::Allow,
                    modify_user_groups: Perm::Allow,
                    ban_users: Perm::Allow,
                    send_messages: Perm::Allow,
                    read_messages: Perm::Allow,
                    manage_messages: Perm::Allow,
                    join_voice: Perm::Allow,
                    view_channel: Perm::Allow,
                },
                name: "admin".to_string(),
                colour: 0,
                position: 0,
            };
            let user = User {
                name: username.to_owned(),
                pfp: CONF.default_pfp.to_owned(),
                uuid: gen_uuid(),
                password: crate::commands::auth::make_hash(password)?,
                groups: vec![group.uuid],
            };

            shared.insert_group(&group)?;
            shared.insert_user(&user)?;
        }
    }
    // debug end
    let state = Arc::new(Mutex::new(shared));

    let addr = format!("{}:{}", &CONF.addr, CONF.port);

    let listener = TcpListener::bind(&addr).await?;
    log::info!("Listening on {}", &addr);

    mainloop(listener, state).await
}

#[cfg(feature = "notls")]
async fn mainloop(listener: TcpListener, state: Arc<Mutex<Shared>>) -> Result<(), Box<dyn Error>> {
    loop {
        let (stream, addr) = listener.accept().await?;
        log::info!("Got connection from {}", &addr);

        let state = Arc::clone(&state);

        tokio::spawn(async move {
            start_worker(stream, state, addr).await;
        });
    }
}

#[cfg(not(feature = "notls"))]
async fn mainloop(listener: TcpListener, state: Arc<Mutex<Shared>>) -> Result<(), Box<dyn Error>> {
    let mut f = File::open(&CONF.certificate_chain).expect("Unable to read certificate chain file");
    let mut chain: Vec<u8> = Vec::new();
    f.read_to_end(&mut chain)
        .expect("Unable to understand the certificate chain. Is it in the right format?");
    let mut f = File::open(&CONF.private_key).expect("Unable to read private key file");
    let mut key: Vec<u8> = Vec::new();
    f.read_to_end(&mut key)
        .expect("Unable to understand the private key. Is it in the right format?");
    let cert = native_tls::Identity::from_pkcs8(&chain, &key).expect(
        "Unable to create TLS identity. Are your certificate and private key valid and correct?",
    );

    let tls_acceptor = tokio_native_tls::TlsAcceptor::from(
        native_tls::TlsAcceptor::builder(cert).build().unwrap(),
    );

    loop {
        let (stream, addr) = listener.accept().await?;
        log::info!("Got connection from {}", &addr);
        let tls_acceptor = tls_acceptor.clone();

        let state = Arc::clone(&state);

        tokio::spawn(async move {
            let tls_stream = tls_acceptor.accept(stream).await.expect("Accept error");
            start_worker(tls_stream, state, addr).await;
        });
    }
}

async fn start_worker(stream: SocketStream, state: Arc<Mutex<Shared>>, addr: std::net::SocketAddr) {
    match process(Arc::clone(&state), stream, addr).await {
        Err(e) => log::error!("An error occurred in the connection:\n{:?}", e),
        Ok(uuid) => {
            let mut state = state.lock().await;
            if let Some(index) = state.peers.iter().position(|x| x.1 == addr) {
                state.peers.remove(index);
            }
            if let Some(uuid) = uuid {
                let count = *state.online.get(&uuid).unwrap_or(&0);
                if count > 0 {
                    state.online.insert(uuid, count - 1);
                }
                if count == 1 {
                    send_online(&state);
                }
            }
        }
    }
    log::info!("Lost connection from {}", &addr);
}

// hang on hang on I can explain
// so basically i have to read the first character of the stream
// to deterime whether it is a websocket connection or a raw socket connection
// however, TlsStream doesn't implement `.peek()`. Hmph. So I had to do this
// attrosity: ShittyStream basically just wraps the stream, implementing
// AsyncRead and AsyncWrite, but the very first character that gets read
// is the one that we read from the original stream in the first place.
// This is a cry for help, please I don't know how to go on
struct ShittyStream {
    c: char,
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

async fn handle_websocket(
    mut websocket: WebSocketStream<TokioIo<Upgraded>>,
    addr: std::net::SocketAddr,
    state: Arc<Mutex<Shared>>,
) -> Result<Option<Uuid>, SError> {
    use hyper_tungstenite::tungstenite::Message;
    let mut peer = make_peer(state.clone(), addr).await?;
    loop {
        tokio::select! {
            result = websocket.next() => match result {
                Some(Ok(msg)) => match msg {
                        Message::Text(msg) =>
                            commands::process_command(&msg, &mut state.lock().await, &mut peer).unwrap(),
                        _ => log::warn!("Got non-text websocket message: {:?}", msg), // TODO handle this properly
                    }
                Some(Err(e)) => log::error!("Error receiving data: {}", e),
                None => break,
            },

            Some(msg) = peer.rx.recv() => websocket.send(Message::Text(msg.to_string())).await.unwrap(),
        }
    }

    Ok(peer.uuid)
}

async fn make_peer(state: Arc<Mutex<Shared>>, addr: std::net::SocketAddr) -> Result<Peer, SError> {
    let peer = Peer::new(addr);
    {
        let mut state = state.lock().await;
        state.peers.push((peer.tx.clone(), peer.addr, peer.uuid));
    }
    let mut json = serde_json::to_value(Response::APIVersionResponse {
        version: API_VERSION,
    })?;
    json["status"] = 200.into();
    peer.tx.send(json)?;
    //TODO handshake protocol
    Ok(peer)
}

async fn process(
    state: Arc<Mutex<Shared>>,
    mut stream: SocketStream,
    addr: std::net::SocketAddr,
) -> Result<Option<Uuid>, SError> {
    // get banned lol
    let banned_ips = vec![std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))];
    if banned_ips.contains(&addr.ip()) {
        return Ok(None);
    }

    //identification of whether a raw socket (json protocol) or websocket has connected
    let mut buf = vec![0; 1];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(None); // must have disconnected or something
    }
    let first_char = char::from_u32(buf[0] as u32).unwrap();

    let stream_with_first_char = ShittyStream {
        c: first_char,
        s: stream,
    };

    if first_char == '{' {
        // Start of a JSON packet, we're using raw sockets
        let mut lines = Framed::new(stream_with_first_char, LinesCodec::new());
        let mut peer = make_peer(state.clone(), addr).await?;

        loop {
            tokio::select! {
                result = lines.next() => match result {
                    Some(Ok(msg)) =>
                        commands::process_command(&msg, &mut state.lock().await, &mut peer)?,
                    Some(Err(e)) => log::error!("Error receiving data: {}", e),
                    None => break,
                },

                Some(msg) = peer.rx.recv() => lines.send(msg.to_string()).await?,
            }
        }
        Ok(peer.uuid)
    } else {
        //?? assume it's websocket
        handle_http_or_websocket(addr, state, stream_with_first_char).await
    }
}

// type SError = Box<dyn std::error::Error + Send + Sync + 'static>;
type SError = anyhow::Error;

async fn handle_http_or_websocket(
    addr: std::net::SocketAddr,
    state: Arc<Mutex<Shared>>,
    stream_with_first_char: ShittyStream,
) -> Result<Option<Uuid>, SError> {
    use hyper::server::conn::http1;
    use hyper::Response;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<Option<Uuid>, SError>>(1);
    let connection = http1::Builder::new()
        .serve_connection(
            hyper_util::rt::TokioIo::new(stream_with_first_char),
            hyper::service::service_fn(|mut request: Request<Incoming>| {
                let new_state = Arc::clone(&state);
                let tx = tx.clone();
                async move {
                    if hyper_tungstenite::is_upgrade_request(&request) {
                        let (response, websocket) = hyper_tungstenite::upgrade(&mut request, None)?;

                        // Spawn a task to handle the websocket connection.
                        tokio::spawn(async move {
                            let websocket = websocket.await.unwrap();
                            tx.send(handle_websocket(websocket, addr, new_state).await)
                                .await
                                .unwrap();
                        });

                        // Return the response so the spawned future can continue.
                        Ok::<hyper::Response<http_body_util::Full<hyper::body::Bytes>>, SError>(
                            response,
                        )
                    } else {
                        // Handle regular HTTP requests here.
                        let _ = tx.try_send(Ok(None));
                        let resp = handle_http(request, new_state).await.unwrap();
                        match resp {
                            HttpResponse::Fine(body) => Ok(Response::new(Full::new(body))),
                            HttpResponse::NotFound => Ok(Response::builder()
                                .status(404)
                                .body(Full::new(hyper::body::Bytes::from("")))
                                .unwrap()),
                        }
                    }
                }
            }),
        )
        .with_upgrades();
    connection.await?;
    let res = rx.recv().await.unwrap_or(Ok(None));
    res
}

enum HttpResponse {
    Fine(hyper::body::Bytes),
    NotFound,
}

async fn handle_http(
    request: Request<Incoming>,
    state: Arc<Mutex<Shared>>,
) -> Result<HttpResponse, SError> {
    use HttpResponse::*;
    let state = state.lock().await;
    match request
        .uri()
        .path()
        .split('/')
        .collect::<Vec<_>>()
        .as_slice()
        .strip_prefix(&[""])
    {
        Some(["api", "v1", "pfp", uuid]) => {
            let Some((name, extension)) = uuid.split_once('.') else {
                return Ok(NotFound);
            };
            if extension != "png" {
                return Ok(NotFound);
            }
            let Ok(uuid) = name.parse::<Uuid>() else {
                return Ok(NotFound);
            };
            let Some(user) = state.get_user(uuid)? else {
                return Ok(NotFound);
            };

            Ok(Fine(
                general_purpose::STANDARD.decode(user.pfp).unwrap().into(),
            ))
        }
        Some(["api", "v1", "icon.png"]) => Ok(Fine(state.get_icon()?.into())),
        _ => Ok(NotFound),
    }
}
