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
    pub rx: mpsc::UnboundedReceiver<serde_json::Value>,
    pub tx: mpsc::UnboundedSender<serde_json::Value>,
    pub user: i64,
    pub addr: SocketAddr,
    pub logged_in: bool,
}

//stupid pun, just takes a couple of things from the peer that can be cloned
#[derive(Clone)]
pub struct Pontoon {
    pub tx: mpsc::UnboundedSender<serde_json::Value>,
    pub addr: SocketAddr,
    pub uuid: i64,
}

impl Pontoon {
    pub fn from_peer(peer: &Peer) -> Self {
        Pontoon {
            tx: peer.tx.clone(),
            addr: peer.addr,
            uuid: peer.user,
        }
    }
}

impl Peer {
    pub async fn new(lines: Framed<TlsStream<TcpStream>, LinesCodec>, addr: SocketAddr) -> std::io::Result<Peer> {
        let (tx, mut rx) = mpsc::unbounded_channel::<serde_json::Value>();

        // let rx = Box::pin(async_stream::stream! {
        //     while let Some(item) = rx.recv().await {
        //         yield item;
        //     }
        // });

        Ok(Peer {
            lines, rx, tx, addr,
            user: NO_UID,
            logged_in: false
        })
    }
}

