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
use std::io::Read;
use rand::prelude::*;

pub mod schema;
pub mod models;

use models::User;
use models::CookedMessage;
use models::Channel;

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

/*
    let handler_state = state.clone();

    ctrlc::set_handler(move || {
        handler_state.save();
        std::process::exit(0); 
    })?;*/

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

fn gen_uuid() -> i64 {
    (random::<u64>() >> 1) as i64
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

struct ServerProperties {
    name: String,
    pfp: String,
}

struct SharedChannel {
    peers: HashMap<SocketAddr, Tx>,
    channel: Channel,
}

struct Shared {
    channels: HashMap<i64, SharedChannel>,
    online: Vec<i64>,
    conn: SqliteConnection,
    properties: ServerProperties,
}

struct Peer {
    lines: Framed<TlsStream<TcpStream>, LinesCodec>,
    rx: Pin<Box<dyn Stream<Item = MessageType> + Send>>,
    channel: i64,
    user: i64,
    addr: SocketAddr,
    logged_in: bool,
}

impl ServerProperties {
    fn load() -> Self {
        let mut server_name = String::new();
        let server_icon: String;
        
        let icon_file = std::fs::File::open("icon.png");
        match icon_file {
            Ok(mut file) => {
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer).unwrap();
                server_icon = base64::encode(buffer);
            }
            Err(_) => {
                panic!("Please provide a file icon.png for the server icon");
            }
        }

        let name_file = std::fs::File::open("name.txt");
        match name_file {
            Ok(mut file) => {
                file.read_to_string(&mut server_name).unwrap();
                server_name.pop();
            }
            Err(_) => {
                panic!("Please provide a file name.txt with the server name");
            }
        }

        ServerProperties {
            name: server_name,
            pfp: server_icon
        }
    }
}

impl Shared {
    fn new() -> Self {
        let channels: HashMap<i64, SharedChannel> = HashMap::new();
        let sqlitedb = SqliteConnection::establish("aster.db").expect(&format!("Error connecting to the database file {}", "aster.db"));
        Shared {
            channels,
            online: Vec::new(),
            conn: sqlitedb,
            properties: ServerProperties::load(),
        }
    }

    fn load(&mut self) {
        let channels = self.get_channels();
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

    // fn save(&mut self) {
        // 
    // }

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
        let new_msg = models::CookedMessageInsertable::new(msg);
        let _ = diesel::insert_into(schema::messages::table)
            .values(&new_msg)
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
                                            uuid: gen_uuid(),
                                            content: message,
                                            author_uuid: self.user,
                                            channel_uuid: self.channel,
                                            date: 0,
                                            rowid: 0,
                                            })))),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        })
    }
}

fn send_metadata(state_lock: &tokio::sync::MutexGuard<'_, Shared>, peer: &Peer) {
    let meta = json::array![state_lock.get_user(&peer.user).as_json()];
    state_lock.channels.get(&peer.channel).unwrap().broadcast(
        peer.addr,
        MessageType::Raw(RawMessage{content: json::object!{command: "metadata", data: meta}.dump()}),
        &state_lock.conn);
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
                send_metadata(&state_lock, peer);
            }

            "/login" => {
                //log in an existing user
                let uuid = argv[1].parse::<i64>().unwrap();
                peer.lines.send(json::object!{"command": "set", "key": "self_uuid", "value": uuid}.dump()).await?;
                peer.user = uuid;
                peer.logged_in = true;
                
                send_metadata(&state_lock, peer);
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
            let mut a = argv[1].parse::<i64>().unwrap();
            //let mut b = argv[2].parse::<usize>().unwrap();
            //if a > history.len() { a = history.len(); }
            //if b > history.len() { b = history.len(); }
            let mut history = schema::messages::table.order(schema::messages::rowid.desc()).limit(a).load::<CookedMessage>(&state_lock.conn).unwrap();
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
                                let state_lock = state.lock().await;
                                state_lock.channels.get(&peer.channel).unwrap().broadcast(
                                    addr, MessageType::Cooked(msg), &state_lock.conn);
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

    Ok(())
}
