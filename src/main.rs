extern crate tokio;
extern crate ctrlc;

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
use std::io::Write;
use std::io::Read;
use rand::prelude::*;

fn write_channel(fname: &str, channel: &SharedChannel) -> std::io::Result<()> {
    let mut f = std::fs::File::create(fname)?;
    f.write_all(channel.as_json().dump().as_bytes())?;
    f.sync_all()?;
    Ok(())
}

fn save(state: Arc<Mutex<Shared>>) -> std::io::Result<()> {
    let mut channels = json::JsonValue::new_array();
    let mut users = json::JsonValue::new_object();

    let state = futures::executor::block_on(state.lock());
    for (name, channel) in &state.channels {
        write_channel(&format!("{}.json", name), &channel)?;
        channels.push(name.to_owned()).unwrap();
    }

    for (uuid, user) in &state.users {
        users[json::stringify(*uuid)] = user.as_json();
    }
    
    let mut channels_file = std::fs::File::create("channels.json")?;
    channels_file.write_all(channels.dump().as_bytes())?;
    channels_file.sync_all()?;
    let mut users_file = std::fs::File::create("users.json")?;
    users_file.write_all(users.dump().as_bytes())?;
    users_file.sync_all()?;
    Ok(())
}

fn load(channels: &mut HashMap<String, SharedChannel>, users: &mut HashMap<u64, User>) {
    match std::fs::read_to_string("channels.json") {
        Ok(content) => {
            match json::parse(&content) {
                Ok(chan_list) => {
                    //load the channels
                    for n in chan_list.members() {
                        match std::fs::read_to_string(format!("{}.json", n.to_string())) {
                            Ok(chan_content) => {
                                match json::parse(&chan_content) {
                                    Ok(chan_content) => {
                                        channels.insert(n.to_string(), SharedChannel::from_json(chan_content));
                                    }
                                    Err(_e) => {
                                        channels.insert(n.to_string(), SharedChannel::new());
                                        println!("Couldn't parse {} channel json file", n.to_string());
                                    }
                                }
                            }
                            Err(_e) => {
                                println!("Couldn't read {} channel json file", n.to_string());
                                channels.insert(n.to_string(), SharedChannel::new());
                            }
                        }
                    }
                }
                Err(_e) => {
                    //default channels
                    println!("Couldn't parse channels json");
                    channels.insert("#general".to_string(), SharedChannel::new());
                }
            }
        }

        Err(_e) => {
            //default channels
            println!("Couldn't read channels.json");
            channels.insert("#general".to_string(), SharedChannel::new());
        }
    }
    match std::fs::read_to_string("users.json") {
        Ok(content) => {
            match json::parse(&content) {
                Ok(user_list) => {
                    //load the channels
                    for (key, val) in user_list.entries() {
                        users.insert(key.parse::<u64>().unwrap(), User::from_json(val));
                    }
                }
                Err(_e) => {
                    //default channels
                    println!("Couldn't parse users json");
                }
            }
        }

        Err(_e) => {
            //default channels
            println!("Couldn't read users.json");
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let state = Arc::new(Mutex::new(Shared::new()));
    save(state.clone()).unwrap();
    let addr = "0.0.0.0:2345".to_string();
    
    let listener = TcpListener::bind(&addr).await?;

    let der = include_bytes!("../identity.pfx");
    let cert = native_tls::Identity::from_pkcs12(der, "").unwrap();

    let tls_acceptor = tokio_native_tls::TlsAcceptor::from(
        native_tls::TlsAcceptor::builder(cert).build().unwrap()
    );

    let handler_state = state.clone();

    ctrlc::set_handler(move || {
        save(handler_state.clone()).unwrap();
        std::process::exit(0); 
    })?;

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

type Tx = mpsc::UnboundedSender<MessageType>;

#[derive(Clone)]
struct User {
    name: String,
    pfp: String,
    uuid: u64,
}

#[derive(Clone)]
struct MessageType {
    content: String,
    user: u64,
}

impl MessageType {
    fn as_json(&self) -> json::JsonValue {
        return json::object!{content: self.content.clone(), user: self.user};
    }
    fn from_json(value: &json::JsonValue) -> Self {
        MessageType {
            content: value["content"].to_string(),
            user: value["user"].as_u64().unwrap(),
        }
    }
}

impl User {
    fn as_json(&self) -> json::JsonValue {
        return json::object!{name: self.name.clone(), uuid: self.uuid, pfp: self.pfp.clone()};
    }
    fn from_json(value: &json::JsonValue) -> Self {
        User {
            name: value["name"].as_str().unwrap().to_string(),
            pfp: value["pfp"].as_str().unwrap().to_string(),
            uuid: value["uuid"].as_u64().unwrap(),
        }
    }
}

struct SharedChannel {
    peers: HashMap<SocketAddr, Tx>,
    history: Vec<MessageType>,
}

struct Shared {
    channels: HashMap<String, SharedChannel>,
    online: Vec<u64>,
    users: HashMap<u64, User>,
}

struct Peer {
    lines: Framed<TlsStream<TcpStream>, LinesCodec>,
    rx: Pin<Box<dyn Stream<Item = MessageType> + Send>>,
    channel: String,
    user: u64,
    addr: SocketAddr,
    logged_in: bool,
}

impl Shared {
    fn new() -> Self {
        let mut channels: HashMap<String, SharedChannel> = HashMap::new();
        let mut users: HashMap<u64, User> = HashMap::new();
        load(&mut channels, &mut users);
        Shared {
            channels,
            online: Vec::new(),
            users,
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

    fn from_json(value: json::JsonValue) -> Self {
        let mut history: Vec<MessageType> = Vec::new();
        for n in value.members() {
            history.push(MessageType::from_json(n));
        }
        SharedChannel {
            peers: HashMap::new(),
            history: history,
        }
    }

    async fn broadcast(&mut self, sender: SocketAddr, message: MessageType) {
        self.history.push(message.clone());
        for peer in self.peers.iter_mut() {
            if *peer.0 != sender {
                let _ = peer.1.send(message.clone());
            }
        }
    }

    fn as_json(&self) -> json::JsonValue {
        let mut arr = json::JsonValue::new_array();
        for msg in self.history.iter() {
            arr.push(msg.as_json()).unwrap();
        }
        return arr;
    }
}

impl Peer {
    async fn new(state: Arc<Mutex<Shared>>, lines: Framed<TlsStream<TcpStream>, LinesCodec>, channel: &String, addr: SocketAddr
    ) -> io::Result<Peer> {
        let (tx, mut rx) = mpsc::unbounded_channel::<MessageType>();
        state.lock().await.channels.get_mut(channel).unwrap().peers.insert(addr, tx);

        let rx = Box::pin(async_stream::stream! {
            while let Some(item) = rx.recv().await {
                yield item;
            }
        });

        let channel = channel.to_owned();
        Ok(Peer {lines, rx, channel, user: u64::MAX, addr, logged_in: false})
    }

    fn channel(&mut self, new_channel: &String, state: &mut tokio::sync::MutexGuard<'_, Shared>) {
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
            return Poll::Ready(Some(Ok(Message::Received(v))));
        }

        let result: Option<_> = futures::ready!(Pin::new(&mut self.lines).poll_next(cx));

        Poll::Ready(match result {
            Some(Ok(message)) => Some(Ok(Message::Broadcast(MessageType{content: message, user: self.user.clone()}))),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        })
    }
}

async fn process_command(msg: &String, state: Arc<Mutex<Shared>>, peer: &mut Peer) -> Result<(), Box<dyn Error>> {
    let split = msg.split(" ");
    let argv = split.collect::<Vec<&str>>();
    let mut state_lock = state.lock().await;

    //commands that can be run when logged in or logged out
    match argv[0] {
        "/get_all_metadata" => {
            let mut meta = json::JsonValue::new_array();
            for (_k, v) in &state_lock.users {
                meta.push(v.as_json()).unwrap();
            }
            peer.lines.send(json::object!{command: "metadata", data: meta}.dump()).await?;
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

                let uuid: u64 = random();
                let user = User{
                    name: json::stringify(uuid),
                    pfp: pfp,
                    uuid: uuid,
                };

                state_lock.users.insert(uuid, user);
                peer.lines.send(json::object!{"command": "set", "key": "self_uuid", "value": uuid}.dump()).await?;
                peer.logged_in = true;
                peer.user = uuid;
            }

            "/login" => {
                //log in an existing user
                let uuid = argv[1].parse::<u64>().unwrap();
                peer.user = uuid;
                peer.logged_in = true;
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
                state_lock.users.get_mut(&peer.user).unwrap().name = argv[1].to_string();
                //state_lock.online.push(peer.user.name.clone());
            }
        }
        
        "/list" => {
            /*
            let mut res = json::JsonValue::new_array();
            for user in state_lock.online.iter() {
                res.push(user).unwrap();
            }
            let final_json = json::object!{
                res: res,
            };
            peer.lines.send(&final_json.dump()).await?;*/
        }

        "/join" => {
            if argv.len() < 2 {
                peer.lines.send("Usage: /join <[#]channel>").await?;
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
                res.push(msg.as_json()).unwrap();
            }
            let json_obj = json::object!{history: res};
            peer.lines.send(&json_obj.dump()).await?;

        }
        //"/createchannel" => {
        //    
        //    shared_lock.channels.insert("#".to_string(), SharedChannel::new());
        //}

        "/leave" => {
            ()
        }
        _ => ()
    }
    Ok(())
}

async fn process(state: Arc<Mutex<Shared>>, stream: TlsStream<TcpStream>, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let channel = "#general".to_string();
        /*
    {
        let mut state = state.lock().await;
        state.online.push(uname.clone());
    }*/

    let lines = Framed::new(stream, LinesCodec::new());
    let mut peer = Peer::new(state.clone(), lines, &channel, addr).await?;
    
    while let Some(result) = peer.next().await {
        match result {
            Ok(Message::Broadcast(msg)) => {
                if msg.content.len() == 0 {
                    continue;
                }
                if msg.content.chars().nth(0).unwrap() == '/' {
                    process_command(&msg.content, state.clone(), &mut peer).await?;
                } else {
                    if peer.logged_in {
                        let mut state_lock = state.lock().await;
                        state_lock.channels.get_mut(&peer.channel).unwrap().broadcast(addr, msg).await;
                    }
                }
            }

            Ok(Message::Received(msg)) => {
                peer.lines.send(&msg.as_json().dump()).await?;
            }

            Err(e) => { println!("Error lmao u figure it out: {}", e); }
        }
    }

    Ok(())
}
