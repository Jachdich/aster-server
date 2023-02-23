use crate::schema;
use crate::models::Emoji;
use crate::helper::{LockedState, JsonValue};
use crate::commands::{Status, Packet};
use crate::Peer;
use crate::CONF;
use serde_json::json;
use diesel::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)] pub struct GetIconPacket;
#[derive(Deserialize)] pub struct GetNamePacket;
#[derive(Deserialize)] pub struct GetMetadataPacket;
#[derive(Deserialize)] pub struct ListChannelsPacket;
#[derive(Deserialize)] pub struct ListEmojiPacket;
#[derive(Deserialize)] pub struct GetEmojiPacket { pub uid: i64 }
#[derive(Deserialize)] pub struct GetUserPacket { pub uuid: i64 }

impl Packet for GetMetadataPacket {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> JsonValue {
        let mut meta: Vec<JsonValue> = Vec::new();
        for v in &state_lock.get_users() {
            meta.push(serde_json::to_value(v).unwrap());
        }
        json!({"command": "metadata", "data": meta, "status": Status::Ok as i32})
    }
}



impl Packet for GetIconPacket {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> JsonValue {
        json!({"command": "get_icon", "data": CONF.icon.to_owned(), "status": Status::Ok as i32})
    }
}

impl Packet for GetNamePacket {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> JsonValue {
        json!({"command": "get_name", "data": CONF.name.to_owned(), "status": Status::Ok as i32})
    }
}

impl Packet for ListChannelsPacket {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> JsonValue {
        let mut res: Vec<JsonValue> = Vec::new();
        let channels = state_lock.get_channels();
        for channel in channels {
            res.push(serde_json::to_value(channel).unwrap());
        }
        
        json!({"command": "list_channels", "data": res, "status": Status::Ok as i32})
    }
}

impl Packet for GetEmojiPacket {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> JsonValue {
        let mut results = schema::emojis::table
            .filter(schema::emojis::uuid.eq(self.uid))
            .limit(1)
            .load::<Emoji>(&mut state_lock.conn).unwrap();
        if results.len() < 1 {
            json!({"command": "get_emoji", "status": Status::NotFound as i32})
        } else {
            json!({"command": "get_emoji", "status": Status::Ok as i32, "data": serde_json::to_value(results.remove(0)).unwrap()})
        }
    }
}

impl Packet for ListEmojiPacket {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> JsonValue {
        let results = schema::emojis::table.load::<Emoji>(&mut state_lock.conn).unwrap();
        json!({"command": "list_emoji", "status": Status::Ok as i32,
            "data": results.iter().map(|res|
                json!({"name": res.name.clone(), "uuid": res.uuid})
            ).collect::<Vec<JsonValue>>()
        })
    }
}
