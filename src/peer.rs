use std::task::{Context, Poll};
use tokio::sync::mpsc;
use std::pin::Pin;
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};
use tokio_stream::Stream;
use tokio_native_tls::TlsStream;
use tokio::net::TcpStream;
use std::net::SocketAddr;
use crate::message::*;
use crate::helper::NO_UID;

pub struct Peer {
    pub lines: Framed<TlsStream<TcpStream>, LinesCodec>,
    pub rx: Pin<Box<dyn Stream<Item = MessageType> + Send>>,
    pub tx: mpsc::UnboundedSender<MessageType>,
    pub user: i64,
    pub addr: SocketAddr,
    pub logged_in: bool,
}

//stupid pun, just takes a couple of things from the peer that can be cloned
#[derive(Clone)]
pub struct Pontoon {
    pub tx: mpsc::UnboundedSender<MessageType>,
    pub addr: SocketAddr,
    pub uuid: i64,
}

impl Pontoon {
    pub fn from_peer(peer: &Peer) -> Self {
        Pontoon {
            tx: peer.tx.clone(),
            addr: peer.addr.clone(),
            uuid: peer.user,
        }
    }
}

impl Peer {
    pub async fn new(lines: Framed<TlsStream<TcpStream>, LinesCodec>, addr: SocketAddr) -> std::io::Result<Peer> {
        let (tx, mut rx) = mpsc::unbounded_channel::<MessageType>();

        let rx = Box::pin(async_stream::stream! {
            while let Some(item) = rx.recv().await {
                yield item;
            }
        });

        Ok(Peer {
            lines, rx, tx, addr,
            user: NO_UID,
            logged_in: false
        })
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
            Some(Ok(message)) => Some(Ok(Message::Broadcast(message))),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        })
    }
}
