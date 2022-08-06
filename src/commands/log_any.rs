use crate::schema;
use crate::models::Emoji;
use crate::helper::LockedState;
use crate::commands::Status;
use diesel::prelude::*;
use crate::CONF;

pub fn get_all_metadata(state_lock: &LockedState) -> json::JsonValue {
    let mut meta = json::JsonValue::new_array();
    for v in &state_lock.get_users() {
        meta.push(v.as_json()).unwrap();
    }
    json::object!{command: "metadata", data: meta}
}

pub fn get_icon(state_lock: &LockedState) -> json::JsonValue {
    json::object!{command: "get_icon", data: CONF.icon.to_owned()}
}

pub fn get_name(state_lock: &LockedState) -> json::JsonValue {
    json::object!{command: "get_name", data: CONF.name.to_owned()}
}

pub fn get_channels(state_lock: &LockedState) -> json::JsonValue {
    let mut res = json::JsonValue::new_array();
    let channels = state_lock.get_channels();
    for channel in channels {
        res.push(channel.as_json()).unwrap();
    }
    
    json::object!{command: "get_channels", data: res}
}

pub fn get_emoji(state_lock: &LockedState, packet: &json::JsonValue) -> json::JsonValue {
    if let Some(uuid) = packet["uid"].as_i64() {
        let results = schema::emojis::table
            .filter(schema::emojis::uuid.eq(uuid))
            .limit(1)
            .load::<Emoji>(&state_lock.conn).unwrap();
        if results.len() < 1 {
            json::object!{command: "get_emoji", code: Status::NotFound as i32}
        } else {
            json::object!{command: "get_emoji", code: Status::Ok as i32, data: results[0].as_json()}
        }
    } else {
        json::object!{command: "get_emoji", code: Status::BadRequest as i32}
    }
}

pub fn list_emoji(state_lock: &LockedState) -> json::JsonValue {
    let results = schema::emojis::table.load::<Emoji>(&state_lock.conn).unwrap();
    json::object!{command: "list_emoji", code: Status::Ok as i32,
        data: results.iter().map(|res|
            json::object!{name: res.name.clone(), uuid: res.uuid}
        ).collect::<Vec<json::JsonValue>>()
    }
}
