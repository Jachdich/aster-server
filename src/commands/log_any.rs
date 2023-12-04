use crate::schema;
use crate::models::Emoji;
use crate::helper::{LockedState, JsonValue};
use crate::commands::{Status, Packet};
use crate::Peer;
use crate::CONF;
use serde_json::json;
use diesel::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)] pub struct GetIconRequest;
#[derive(Deserialize)] pub struct GetNameRequest;
#[derive(Deserialize)] pub struct GetMetadataRequest;
#[derive(Deserialize)] pub struct ListChannelsRequest;
#[derive(Deserialize)] pub struct ListEmojiRequest;
#[derive(Deserialize)] pub struct GetEmojiRequest { pub uuid: i64 }
#[derive(Deserialize)] pub struct GetUserRequest { pub uuid: i64 }

impl Packet for GetMetadataRequest {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> JsonValue {
        let mut meta: Vec<JsonValue> = Vec::new();
        for v in &state_lock.get_users() {
            meta.push(serde_json::to_value(v).unwrap());
        }
        json!({"command": "metadata", "data": meta, "status": Status::Ok as i32})
    }
}

impl Packet for GetUserRequest {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> JsonValue {
        match state_lock.get_user(&self.uuid) {
            Ok(Some(peer_meta)) => {
                let meta = serde_json::to_value(peer_meta).unwrap();
                json!({"command": "get_user", "data": meta, "status": Status::Ok as i32})
            },
            Ok(None) => json!({"command": "get_user", "status": Status::NotFound as i32}),
            Err(e) => {
                println!("Warn(GetUserPacket::execute): Error getting user metadata: {:?}", e);
                json!({"command": "get_user", "status": Status::InternalError as i32})
            }
        }
    }
}


impl Packet for GetIconRequest {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> JsonValue {
        json!({"command": "get_icon", "data": CONF.icon.to_owned(), "status": Status::Ok as i32})
    }
}

impl Packet for GetNameRequest {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> JsonValue {
        json!({"command": "get_name", "data": CONF.name.to_owned(), "status": Status::Ok as i32})
    }
}

impl Packet for ListChannelsRequest {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> JsonValue {
        let mut res: Vec<JsonValue> = Vec::new();
        let channels = state_lock.get_channels();
        for channel in channels {
            res.push(serde_json::to_value(channel).unwrap());
        }
        
        json!({"command": "list_channels", "data": res, "status": Status::Ok as i32})
    }
}

impl Packet for GetEmojiRequest {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> JsonValue {
        let mut results = schema::emojis::table
            .filter(schema::emojis::uuid.eq(self.uuid))
            .limit(1)
            .load::<Emoji>(&mut state_lock.conn).unwrap();
        if results.is_empty() {
            json!({"command": "get_emoji", "status": Status::NotFound as i32})
        } else {
            json!({"command": "get_emoji", "status": Status::Ok as i32, "data": serde_json::to_value(results.remove(0)).unwrap()})
        }
    }
}

impl Packet for ListEmojiRequest {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> JsonValue {
        let results = schema::emojis::table.load::<Emoji>(&mut state_lock.conn).unwrap();
        json!({"command": "list_emoji", "status": Status::Ok as i32,
            "data": results.iter().map(|res|
                json!({"name": res.name.clone(), "uuid": res.uuid})
            ).collect::<Vec<JsonValue>>()
        })
    }
}
