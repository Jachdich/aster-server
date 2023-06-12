use std::net::SocketAddr;
use tokio::sync::mpsc;

pub struct Peer {
    pub rx: mpsc::UnboundedReceiver<serde_json::Value>,
    pub tx: mpsc::UnboundedSender<serde_json::Value>,
    pub uuid: Option<i64>,
    pub addr: SocketAddr,
}

impl Peer {
    pub fn new(addr: SocketAddr) -> Peer {
        let (tx, rx) = mpsc::unbounded_channel::<serde_json::Value>();

        // let rx = Box::pin(async_stream::stream! {
        //     while let Some(item) = rx.recv().await {
        //         yield item;
        //     }
        // });

        Peer {
            rx,
            tx,
            addr,
            uuid: None,
        }
    }

    pub fn logged_in(&self) -> bool {
        self.uuid.is_some()
    }
}
