extern crate tokio;
extern crate ctrlc;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::{Stream, StreamExt};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};
use tokio_native_tls::{TlsStream};

#[macro_use]
extern crate diesel;
use diesel::prelude::*;

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

pub mod schema;
pub mod models;

use models::User;
use models::CookedMessage;
use models::Channel;

/*
fn write_channel(fname: &str, channel: &SharedChannel) -> std::io::Result<()> {
    let mut f = std::fs::File::create(fname)?;
    f.write_all(channel.as_json().dump().as_bytes())?;
    f.sync_all()?;
    Ok(())
}*/

fn save(state: Arc<Mutex<Shared>>) -> std::io::Result<()> {
/*
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
    users_file.sync_all()?;*/
    Ok(())
}

fn load(channels: &mut HashMap<i64, SharedChannel>, db: &mut SqliteConnection) {
    
/*
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
    }*/
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let state = Arc::new(Mutex::new(Shared::new()));

    {
        let mut state = state.lock().await;
        state.load();
    }
    
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
struct RawMessage {
    content: String,
}

#[derive(Clone)]
enum MessageType {
    Raw(RawMessage),
    Cooked(CookedMessage),
}

/*
impl Group {
    fn as_json(&self) -> json::JsonValue {
        return json::object!{name: self.name.clone(), perms: self.perms, uuid: self.uuid};
    }
    fn from_json(value: &json::JsonValue) -> Self {
        Group {
            name: value["name"].to_string(),
            perms: value["perms"].
        }
    }
}*/

struct SharedChannel {
    peers: HashMap<SocketAddr, Tx>,
    channel: Channel,
}

struct Shared {
    channels: HashMap<i64, SharedChannel>,
    online: Vec<i64>,
    conn: SqliteConnection,
}

struct Peer {
    lines: Framed<TlsStream<TcpStream>, LinesCodec>,
    rx: Pin<Box<dyn Stream<Item = MessageType> + Send>>,
    channel: i64,
    user: i64,
    addr: SocketAddr,
    logged_in: bool,
}

impl Shared {
    fn new() -> Self {
        let mut channels: HashMap<i64, SharedChannel> = HashMap::new();
        let mut sqlitedb = SqliteConnection::establish("aster.db").expect(&format!("Error connecting to the database file {}", "aster.db"));
        Shared {
            channels,
            online: Vec::new(),
            conn: sqlitedb,
        }
    }

    fn load(&mut self) {
        let mut channels = self.get_channels();
        if channels.len() == 0 {
            let new_channel = Channel::new("general");
            self.channels.insert(new_channel.uuid, SharedChannel::new(new_channel.clone()));
            self.insert_channel(new_channel);
        } else {
            for channel in channels {
                self.channels.insert(channel.uuid, SharedChannel::new(channel));
            }
        }
    }

    fn get_users(&self) -> Vec<User> {
        return schema::users::table.load::<User>(&self.conn).unwrap();
    }

    fn get_channels(&self) -> Vec<Channel> {
        return schema::channels::table.load::<Channel>(&self.conn).unwrap();
    }

    fn get_user(&self, user: &i64) -> User {
        let mut results = schema::users::table
            .filter(schema::users::uuid.eq(user))
            .limit(1)
            .load::<User>(&self.conn)
            .expect("User does not exist");

        return results.remove(0);
    }

    fn get_channel(&self, channel: &i64) -> Channel {
        let mut results = schema::channels::table
            .filter(schema::channels::uuid.eq(channel))
            .limit(1)
            .load::<Channel>(&self.conn)
            .expect("Channel does not exist");

        return results.remove(0);
    }

    fn get_channel_by_name(&self, channel: &String) -> Channel {
        let mut results = schema::channels::table
            .filter(schema::channels::name.eq(channel))
            .limit(1)
            .load::<Channel>(&self.conn)
            .expect("Channel does not exist");

        return results.remove(0);
    }

    fn insert_channel(&self, channel: Channel) {
        let _ = diesel::insert_into(schema::channels::table)
            .values(&channel)
            .execute(&self.conn)
            .expect("Error appending channel");
    }

    fn insert_user(&self, user: User) {
        let _ = diesel::insert_into(schema::users::table)
            .values(&user)
            .execute(&self.conn)
            .expect("Error appending user");
    }

    fn update_user(&self, user: User) {
        diesel::update(schema::users::table.find(user.uuid))
            .set((schema::users::name.eq(user.name),
            schema::users::pfp.eq(user.pfp),
            schema::users::group_uuid.eq(user.group_uuid)))
            .execute(&self.conn)
            .expect(&format!("Unable to find user {}", user.uuid));
    }
}

impl SharedChannel {
    fn new(channel: Channel) -> Self {
        SharedChannel {
            peers: HashMap::new(),
            channel,             
        }
    }

    fn broadcast(&self, sender: SocketAddr, message: MessageType, conn: &SqliteConnection) {
        match &message {
            MessageType::Cooked(msg) => {
                self.add_to_history(msg.clone(), conn);
                for peer in self.peers.iter() {
                    if *peer.0 != sender {
                        let _ = peer.1.send(message.clone());
                    }
                }

            }
            MessageType::Raw(_) => {
                for peer in self.peers.iter() {
                    let _ = peer.1.send(message.clone());
                }
            }
        }
    }

    fn add_to_history(&self, msg: CookedMessage, conn: &SqliteConnection) {
        let _ = diesel::insert_into(schema::messages::table)
            .values(&msg)
            .execute(conn)
            .expect("Error appending to history");
    }
}

impl Peer {
    async fn new(state: Arc<Mutex<Shared>>, lines: Framed<TlsStream<TcpStream>, LinesCodec>, channel: i64, addr: SocketAddr
    ) -> io::Result<Peer> {
        let (tx, mut rx) = mpsc::unbounded_channel::<MessageType>();
        state.lock().await.channels.get_mut(&channel).unwrap().peers.insert(addr, tx);

        let rx = Box::pin(async_stream::stream! {
            while let Some(item) = rx.recv().await {
                yield item;
            }
        });

        Ok(Peer {lines, rx, channel, user: i64::MAX, addr, logged_in: false})
    }

    fn channel(&mut self, new_channel: i64, state: &mut tokio::sync::MutexGuard<'_, Shared>) {
        let tx = state.channels.get_mut(&self.channel).unwrap().peers.get(&self.addr).unwrap().clone();
        state.channels.get_mut(&self.channel).unwrap().peers.remove(&self.addr);
        state.channels.get_mut(&new_channel).unwrap().peers.insert(self.addr, tx);
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
            Some(Ok(message)) => Some(Ok(Message::Broadcast(
                                         MessageType::Cooked(CookedMessage{
                                            uuid: random::<i64>(),
                                            content: message,
                                            author_uuid: self.user,
                                            channel_uuid: self.channel,
                                            date: 0
                                            })))),
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
            for v in &state_lock.get_users() {
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

                let uuid: i64 = random();
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
            }

            "/login" => {
                //log in an existing user
                let uuid = argv[1].parse::<i64>().unwrap();
                peer.lines.send(json::object!{"command": "set", "key": "self_uuid", "value": uuid}.dump()).await?;
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
                let mut user = state_lock.get_user(&peer.user);
                user.name = argv[1].to_string();
                state_lock.update_user(user);
                let meta = json::array![state_lock.get_user(&peer.user).as_json()];
                state_lock.channels.get(&peer.channel).unwrap().broadcast(
                    peer.addr,
                    MessageType::Raw(RawMessage{content: json::object!{command: "metadata", data: meta}.dump()}),
                    &state_lock.conn);
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
            } else {
                peer.channel(state_lock.get_channel_by_name(&argv[1].to_string()).uuid, &mut state_lock);
            }
        }

        "/history" => {
            /*
            let history = &state_lock.channels.get(&peer.channel).unwrap();
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
            peer.lines.send(&json_obj.dump()).await?;*/

        }

        "/pfp" => {
            if argv.len() < 2 {
                peer.lines.send("Usage: /pfp <base64 encoded PNG file>").await?;
                return Ok(());
            }
            let mut user = state_lock.get_user(&peer.user);
            user.pfp = argv[1].to_string();
            state_lock.update_user(user);

            let meta = json::array![state_lock.get_user(&peer.user).as_json()];
            state_lock.channels.get(&peer.channel).unwrap().broadcast(
                peer.addr,
                MessageType::Raw(RawMessage{content: json::object!{command: "metadata", data: meta}.dump()}),
                &state_lock.conn);

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
    let channel: i64;

    {
        let mut state = state.lock().await;
        channel = state.get_channel_by_name(&"general".to_string()).uuid;
    }
        /*
    {
        let mut state = state.lock().await;
        state.online.push(uname.clone());
    }*/

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
                                let mut state_lock = state.lock().await;
                                state_lock.channels.get(&peer.channel).unwrap().broadcast(
                                    addr, MessageType::Cooked(msg), &state_lock.conn);
                            }
                        }
                    }
                    MessageType::Raw(msg) => {
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

    Ok(())
}
