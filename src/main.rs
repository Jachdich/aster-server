extern crate tokio;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::{Stream, StreamExt};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

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
    let addr = "127.0.0.1:2345".to_string();
    
    let listener = TcpListener::bind(&addr).await?;

    loop {
        let (stream, addr) = listener.accept().await?;

        let state = Arc::clone(&state);

        tokio::spawn(async move {
            if let Err(e) = process(state, stream, addr).await {
                println!("An error occurred: {:?}", e);
            }
        });
    }
}

type Tx = mpsc::UnboundedSender<String>;

struct SharedChannel {
    peers: HashMap<SocketAddr, Tx>,
    online: Vec<String>,
}

struct Shared {
    channels: HashMap<String, SharedChannel>,
}

struct Peer {
    lines: Framed<TcpStream, LinesCodec>,
    rx: Pin<Box<dyn Stream<Item = String> + Send>>, //TODO this is not what we want to do!
    channel: String,
    addr: SocketAddr,
}

impl Shared {
    fn new() -> Self {
        let mut channels: HashMap<String, SharedChannel> = HashMap::new();
        channels.insert("#general".to_string(), SharedChannel::new());
        channels.insert("#memes".to_string(), SharedChannel::new());
        Shared {
            //channels: [("#general", SharedChannel::new()), ("#other", SharedChannel::new())].iter().cloned().collect()
            channels
        }
    }
}

impl SharedChannel {
    fn new() -> Self {
        SharedChannel {
            peers: HashMap::new(),
            online: Vec::new(),
        }
    }

    async fn broadcast(&mut self, sender: SocketAddr, message: &str) {
        for peer in self.peers.iter_mut() {
            if *peer.0 != sender {
                let _ = peer.1.send(message.into());
            }
        }
    }
}

impl Peer {
    async fn new(state: Arc<Mutex<Shared>>, lines: Framed<TcpStream, LinesCodec>, channel: &String
    ) -> io::Result<Peer> {
        let addr = lines.get_ref().peer_addr()?;
        let (tx, mut rx) = mpsc::unbounded_channel();
        state.lock().await.channels.get_mut(channel).unwrap().peers.insert(addr, tx);

        let rx = Box::pin(async_stream::stream! {
            while let Some(item) = rx.recv().await {
                yield item;
            }
        });

        let channel = channel.to_owned();
        Ok(Peer {lines, rx, channel, addr})
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
    Broadcast(String),
    Received(String),
}

impl Stream for Peer {
    type Item = Result<Message, LinesCodecError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {

        if let Poll::Ready(Some(v)) = Pin::new(&mut self.rx).poll_next(cx) {
            return Poll::Ready(Some(Ok(Message::Received(v))));
        }

        let result: Option<_> = futures::ready!(Pin::new(&mut self.lines).poll_next(cx));

        Poll::Ready(match result {
            Some(Ok(message)) => Some(Ok(Message::Broadcast(message))),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        })
    }
}

async fn process(state: Arc<Mutex<Shared>>, stream: TcpStream, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let mut channel = "#general".to_string();

    let lines = Framed::new(stream, LinesCodec::new());
    let mut peer = Peer::new(state.clone(), lines, &channel).await?;

    let mut uname = format!("{}", addr);
    
    {
        //let mut state = state.lock().await;
        //state.online.push(uname.clone());
    }

    while let Some(result) = peer.next().await {
        match result {
            Ok(Message::Broadcast(msg)) => {
                let mut state_lock = state.lock().await;
                if msg.chars().nth(0).unwrap() == '/' {
                    let split = msg.split(" ");
                    let argv = split.collect::<Vec<&str>>();
                    match argv[0] {
                        "/nick" => {
                            //let index = state.online.iter().position(|x| *x == uname).unwrap();
                            //state.online.remove(index);
                            uname = argv[1].to_string();
                            //state.online.push(uname.clone());
                        }
/*
                        "/list" => {
                            let mut res = "".to_string();
                            for user in state.online.iter() {
                                res.push_str(user);
                                res.push_str(&", ");
                            }
                            peer.lines.send(&res).await?;
                        }*/

                        "/join" => {
                            channel = argv[1].to_string();
                            peer.channel(&channel, &mut state_lock);
                        }
                        _ => ()
                    }
                } else {
                    let msg = format!("{}: {}", uname, msg);
                    state_lock.channels.get_mut(&channel).unwrap().broadcast(addr, &msg).await;
                }
            }

            Ok(Message::Received(msg)) => {
                peer.lines.send(&msg).await?;
            }

            Err(e) => { println!("Error lmao u figure it out"); }
        }
    }

    Ok(())
}
