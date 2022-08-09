use crate::schema;
use crate::models::Emoji;
use crate::helper::{LockedState, JsonValue};
use crate::commands::Status;
use diesel::prelude::*;
use crate::CONF;
use serde_json::json;

pub fn get_all_metadata(state_lock: &LockedState) -> JsonValue {
    let mut meta: Vec<JsonValue> = Vec::new();
    for v in &state_lock.get_users() {
        meta.push(serde_json::to_value(v).unwrap());
    }
    json!({"command": "metadata", "data": meta})
}

pub fn get_icon(state_lock: &LockedState) -> JsonValue {
    json!({"command": "get_icon", "data": CONF.icon.to_owned()})
}

pub fn get_name(state_lock: &LockedState) -> JsonValue {
    json!({"command": "get_name", "data": CONF.name.to_owned()})
}

pub fn get_channels(state_lock: &LockedState) -> JsonValue {
    let mut res: Vec<JsonValue> = Vec::new();
    let channels = state_lock.get_channels();
    for channel in channels {
        res.push(serde_json::to_value(channel).unwrap());
    }
    
    json!({"command": "get_channels", "data": res})
}

pub fn get_emoji(state_lock: &LockedState, packet: &JsonValue) -> JsonValue {
    if let Some(uuid) = packet["uid"].as_i64() {
        let mut results = schema::emojis::table
            .filter(schema::emojis::uuid.eq(uuid))
            .limit(1)
            .load::<Emoji>(&state_lock.conn).unwrap();
        if results.len() < 1 {
            json!({"command": "get_emoji", "code": Status::NotFound as i32})
        } else {
            json!({"command": "get_emoji", "code": Status::Ok as i32, "data": serde_json::to_value(results.remove(0)).unwrap()})
        }
    } else {
        json!({"command": "get_emoji", "code": Status::BadRequest as i32})
    }
}

pub fn list_emoji(state_lock: &LockedState) -> JsonValue {
    let results = schema::emojis::table.load::<Emoji>(&state_lock.conn).unwrap();
    json!({"command": "list_emoji", "code": Status::Ok as i32,
        "data": results.iter().map(|res|
            json!({"name": res.name.clone(), "uuid": res.uuid})
        ).collect::<Vec<JsonValue>>()
    })
}
