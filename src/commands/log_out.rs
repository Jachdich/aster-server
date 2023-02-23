use crate::commands::{send_metadata, send_online};
use crate::commands::{Packet, Status};
use crate::helper::{gen_uuid, JsonValue, LockedState};
use crate::models::User;
use crate::Peer;
use crate::CONF;

use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct RegisterPacket {
    pub passwd: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct LoginPacket {
    pub passwd: String,
    pub uname: Option<String>,
    pub uuid: Option<i64>,
}

impl Packet for RegisterPacket {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if peer.logged_in {
            //registering doesn't make sense when logged in
            return json!({"command": "register", "status": Status::MethodNotAllowed as i32});
        }

        let uuid = gen_uuid();
        let user = User {
            name: self.name.to_owned(),
            pfp: CONF.default_pfp.to_owned(),
            uuid,
            group_uuid: 0,
        };

        match state_lock.insert_user(user) {
            Err(_) => return json!({"command": "register", "status": Status::InternalError as i32}),
            _ => (),
        }
        peer.logged_in = true;
        peer.user = uuid;

        state_lock.inc_online(peer.user);

        send_metadata(state_lock, peer);
        send_online(state_lock);

        json!({"command": "register", "status": Status::Ok as i32, "uuid": uuid})
    }
}

impl Packet for LoginPacket {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> JsonValue {
        if peer.logged_in {
            //logging in doesn't make sense when already logged in
            return json!({"command": "login", "status": Status::MethodNotAllowed as i32});
        }

        let uuid = if let Some(uname) = &self.uname {
            if let Some(user) = state_lock.get_user_by_name(uname) {
                user.uuid
            } else {
                return json!({"command": "login", "status": Status::NotFound as i32});
            }
        } else if let Some(uuid) = self.uuid {
            uuid
        } else {
            //neither uname nor uuid were provided
            return json!({"command": "login", "status": Status::BadRequest as i32});
        };

        //TODO confirm password
        peer.user = uuid;
        peer.logged_in = true;

        state_lock.inc_online(peer.user);
        send_metadata(state_lock, peer);
        send_online(state_lock);
        json!({"command": "login", "status": Status::Ok as i32, "uuid": uuid})
    }
}
