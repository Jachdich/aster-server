extern crate tokio;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::{Stream, StreamExt};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};
use tokio_native_tls::{TlsStream};

use futures::SinkExt;
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let state = Arc::new(Mutex::new(Shared::new()));
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
            let mut tls_stream = tls_acceptor.accept(stream).await.expect("Accept error");
            if let Err(e) = process(state, tls_stream, addr).await {
                println!("An error occurred: {:?}", e);
            }
        });
    }
}

type Tx = mpsc::UnboundedSender<String>;

#[derive(Clone)]
struct User {
    name: String,
}

struct MessageType {
    content: String,
    user: User,
}

struct SharedChannel {
    peers: HashMap<SocketAddr, Tx>,
    history: Vec<MessageType>,
}

struct Shared {
    channels: HashMap<String, SharedChannel>,
    online: Vec<String>,
}

struct Peer {
    lines: Framed<TlsStream<TcpStream>, LinesCodec>,
    rx: Pin<Box<dyn Stream<Item = String> + Send>>, //TODO this is not what we want to do!
    channel: String,
    user: User,
    addr: SocketAddr,
}

impl Shared {
    fn new() -> Self {
        let mut channels: HashMap<String, SharedChannel> = HashMap::new();
        channels.insert("#general".to_string(), SharedChannel::new());
        channels.insert("#memes".to_string(), SharedChannel::new());
        Shared {
            channels,
            online: Vec::new(),
        }
    }
}

impl SharedChannel {
    fn new() -> Self {
        SharedChannel {
            peers: HashMap::new(),
            history: Vec::new(),
        }
    }

    async fn broadcast(&mut self, sender: SocketAddr, message: MessageType) {
        let msg = json::object!{username: message.user.name.clone(), message: message.content.clone()};
        let msg_string = msg.dump();
        self.history.push(message);
        for peer in self.peers.iter_mut() {
            if *peer.0 != sender {
                let _ = peer.1.send(msg_string.clone());
            }
        }
    }
}

impl Peer {
    async fn new(state: Arc<Mutex<Shared>>, lines: Framed<TlsStream<TcpStream>, LinesCodec>, channel: &String, uname: &String
    ) -> io::Result<Peer> {
        let addr = lines.get_ref().get_ref().get_ref().get_ref().peer_addr()?;
        let (tx, mut rx) = mpsc::unbounded_channel();
        state.lock().await.channels.get_mut(channel).unwrap().peers.insert(addr, tx);

        let rx = Box::pin(async_stream::stream! {
            while let Some(item) = rx.recv().await {
                yield item;
            }
        });

        let channel = channel.to_owned();
        let user = User{name: uname.to_owned()};
        Ok(Peer {lines, rx, channel, user, addr})
    }

    fn channel(&mut self, new_channel: &String, state: &mut tokio::sync::MutexGuard<'_, Shared>) {
        //let channels = &mut ;
        //let addr = self.lines.get_ref().peer_addr().unwrap();
        let tx = state.channels.get_mut(&self.channel).unwrap().peers.get(&self.addr).unwrap().clone();
        state.channels.get_mut(&self.channel).unwrap().peers.remove(&self.addr);
        state.channels.get_mut(new_channel).unwrap().peers.insert(self.addr, tx);
        self.channel = new_channel.to_owned();
    }
}

enum Message {
    Broadcast(MessageType),
    Received(MessageType),
}

impl Stream for Peer {
    type Item = Result<Message, LinesCodecError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {

        if let Poll::Ready(Some(v)) = Pin::new(&mut self.rx).poll_next(cx) {
            return Poll::Ready(Some(Ok(Message::Received(MessageType{content: v, user: self.user.clone()}))));
        }

        let result: Option<_> = futures::ready!(Pin::new(&mut self.lines).poll_next(cx));

        Poll::Ready(match result {
            Some(Ok(message)) => Some(Ok(Message::Broadcast(MessageType{content: message, user: self.user.clone()}))),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        })
    }
}

async fn process_command(msg: &String, state: Arc<Mutex<Shared>>, peer: &mut Peer) {
    let split = msg.split(" ");
    let argv = split.collect::<Vec<&str>>();
    let mut state_lock = state.lock().await;
    match argv[0] {
        "/nick" => {
            if argv.len() < 2 {
                peer.lines.send("Usage: /nick <nickname>").await;
            } else {
                let index = state_lock.online.iter().position(|x| *x == peer.user.name).unwrap();
                state_lock.online.remove(index);
                peer.user.name = argv[1].to_string();
                state_lock.online.push(peer.user.name.clone());
            }
        }
        
        "/list" => {
            let mut res = json::JsonValue::new_array();
            for user in state_lock.online.iter() {
                res.push(user.to_owned());
            }
            let final_json = json::object!{
                message: res.dump(),
                username: "server"
            };
            peer.lines.send(&final_json.dump()).await;
        }

        "/join" => {
            if argv.len() < 2 {
                peer.lines.send("Usage: /join <[#]channel>").await;

            }
            peer.channel(&argv[1].to_string(), &mut state_lock);
        }

        "/history" => {
            let history = &state_lock.channels.get(&peer.channel).unwrap().history;
            let mut a = argv[1].parse::<usize>().unwrap();
            let mut b = argv[2].parse::<usize>().unwrap();
            if a > history.len() { a = history.len(); }
            if b > history.len() { b = history.len(); }

            let mut res = json::JsonValue::new_array();

            for msg in history[history.len() - b..history.len() - a].iter() {
                //peer.lines.send(msg).await;
                //res.push(msg.to_owne)
            }
            
        }

        

        //"/createchannel" => {
        //    
        //    shared_lock.channels.insert("#".to_string(), SharedChannel::new());
        //}

        "/leave" => {
            return;
        }
        _ => ()
    }
}

async fn process(state: Arc<Mutex<Shared>>, stream: TlsStream<TcpStream>, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let mut channel = "#general".to_string();

    let mut uname = format!("{}", addr);
    {
        let mut state = state.lock().await;
        state.online.push(uname.clone());
    }

    let lines = Framed::new(stream, LinesCodec::new());
    let mut peer = Peer::new(state.clone(), lines, &channel, &uname).await?;
    
    while let Some(result) = peer.next().await {
        match result {
            Ok(Message::Broadcast(msg)) => {
                if msg.content.len() == 0 {
                    continue;
                }
                if msg.content.chars().nth(0).unwrap() == '/' {
                    process_command(&msg.content, state.clone(), &mut peer).await;
                } else {
                    let mut state_lock = state.lock().await;
                    state_lock.channels.get_mut(&channel).unwrap().broadcast(addr, msg).await;
                }
            }

            Ok(Message::Received(msg)) => {
                let msg = json::object!{username: msg.user.name.clone(), message: msg.content.clone()};
                let msg_string = msg.dump();
                peer.lines.send(&msg_string).await?;
            }

            Err(e) => { println!("Error lmao u figure it out: {}", e); }
        }
    }

    Ok(())
}
