use crate::message::MessageType;
use crate::schema;
use diesel::prelude::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use crate::helper::Tx;
use crate::models::Channel;
use crate::shared::Shared;
use crate::message::*;

pub struct SharedChannel {
    pub peers: HashMap<SocketAddr, Tx>,
    pub channel: Channel,
    pub vcpeers: HashMap<SocketAddr, Tx>,
}

impl SharedChannel {
    pub fn new(channel: Channel) -> Self {
        SharedChannel {
            peers: HashMap::<SocketAddr, Tx>::new(),
            vcpeers: HashMap::<SocketAddr, Tx>::new(),
            channel,
        }
    }

    pub fn broadcast(&self, sender: SocketAddr, message: MessageType, state: &tokio::sync::MutexGuard<'_, Shared>) {
        match &message {
            MessageType::Cooked(msg) => {
                self.add_to_history(msg.clone(), &state.conn);
                for peer in self.peers.iter() {
                    if *peer.0 != sender {
                        let _ = peer.1.send(message.clone());
                    }
                }
                state.broadcast_unread(self.channel.uuid, state);
            }

            MessageType::Raw(_) => {
                for peer in self.peers.iter() {
                    let _ = peer.1.send(message.clone());
                }
            }

            MessageType::Internal(_) => {
                for peer in self.peers.iter() {
                    let _ = peer.1.send(message.clone());
                }
            }
        }
    }

    pub fn broadcast_vc(&self, sender: SocketAddr, message: RawMessage) {
        for peer in self.vcpeers.iter() {
            let _ = peer.1.send(MessageType::Raw(message.clone()));
        }
    }

    pub fn add_to_history(&self, msg: CookedMessage, conn: &SqliteConnection) {
        let new_msg = CookedMessageInsertable::new(msg);
        let _ = diesel::insert_into(schema::messages::table)
            .values(&new_msg)
            .execute(conn)
            .expect("Error appending to history");
    }
}