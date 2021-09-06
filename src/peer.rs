use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use crate::helper::gen_uuid;
use std::pin::Pin;
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};
use tokio_stream::Stream;
use tokio_native_tls::TlsStream;
use tokio::net::TcpStream;
use std::net::SocketAddr;
use std::sync::Arc;
use chrono;
use crate::shared::Shared;
use crate::message::*;

pub struct Peer {
    pub lines: Framed<TlsStream<TcpStream>, LinesCodec>,
    pub rx: Pin<Box<dyn Stream<Item = MessageType> + Send>>,
    tx: mpsc::UnboundedSender<MessageType>,
    pub channel: i64,
    pub vcchannel: i64,
    pub user: i64,
    pub addr: SocketAddr,
    pub logged_in: bool,
}

impl Peer {
    pub async fn new(state: Arc<Mutex<Shared>>, lines: Framed<TlsStream<TcpStream>, LinesCodec>, channel: i64, addr: SocketAddr
    ) -> std::io::Result<Peer> {
        let (tx, mut rx) = mpsc::unbounded_channel::<MessageType>();
        state.lock().await.channels.get_mut(&channel).unwrap().peers.insert(addr, tx.clone());

        let rx = Box::pin(async_stream::stream! {
            while let Some(item) = rx.recv().await {
                yield item;
            }
        });

        Ok(Peer {lines, rx, tx, channel, vcchannel: i64::MAX, user: i64::MAX, addr, logged_in: false})
    }

    pub fn channel(&mut self, new_channel: i64, state: &mut tokio::sync::MutexGuard<'_, Shared>) {
        //let tx = state.channels.get_mut(&self.channel).unwrap().peers.get(&self.addr).unwrap().clone();
        state.channels.get_mut(&self.channel).unwrap().peers.remove(&self.addr);
        state.channels.get_mut(&new_channel).unwrap().peers.insert(self.addr, self.tx.clone());
        self.channel = new_channel.to_owned();
    }

    pub fn vcchannel(&mut self, chan: i64, state: &mut tokio::sync::MutexGuard<'_, Shared>) {
        if self.vcchannel != i64::MAX {
            state.channels.get_mut(&self.channel).unwrap().peers.remove(&self.addr);
        }
        if chan != i64::MAX {
            state.channels.get_mut(&chan).unwrap().peers.insert(self.addr, self.tx.clone());
        }
        self.vcchannel = chan.to_owned();
    }
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
                                            date: chrono::offset::Utc::now().timestamp() as i32,
                                            rowid: 0,
                                            })))),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        })
    }
}
