use crate::helper::{gen_uuid, LockedState};
use crate::message::Message;
use crate::models::{SyncData, SyncServer};
use crate::peer::Peer;
use crate::schema;
use crate::{
    commands::{
        send_metadata, CmdError, Request,
        Response::{self, *},
        Status,
    },
    helper::Uuid,
};
use serde::Deserialize;

use super::auth::make_hash;

#[derive(Deserialize)]
pub struct SendRequest {
    pub content: String,
    pub channel: i64,
    pub reply: Option<i64>,
}
#[derive(Deserialize)]
pub struct HistoryRequest {
    pub num: u32,
    pub channel: i64,
    pub before_message: Option<i64>,
}

#[derive(Deserialize)]
pub struct SyncSetRequest {
    pub uname: String,
    pub pfp: String,
}

#[derive(Deserialize)]
pub struct SyncSetServersRequest {
    pub servers: Vec<SyncServer>,
}

#[derive(Deserialize)]
pub struct NickRequest {
    pub nick: String,
}

#[derive(Deserialize)]
pub struct OnlineRequest;

#[derive(Deserialize)]
pub struct PfpRequest {
    pub data: String,
}

#[derive(Deserialize)]
pub struct SyncGetRequest;

#[derive(Deserialize)]
pub struct SyncGetServersRequest;

#[derive(Deserialize)]
pub struct EditRequest {
    pub message: Uuid,
    pub new_content: String,
}

#[derive(Deserialize)]
pub struct DeleteRequest {
    pub message: Uuid,
}

#[derive(Deserialize)]
pub struct PasswordChangeRequest {
    pub new_password: String,
}

impl Request for PasswordChangeRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::MethodNotAllowed));
        }
        let Some(mut user) = state_lock.get_user(&peer.uuid.unwrap())? else {
            return Ok(GenericResponse(Status::NotFound));
        };
        user.password = make_hash(&self.new_password)?;
        state_lock.update_user(&user)?;

        Ok(GenericResponse(Status::Ok))
    }
}

impl Request for EditRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }
        let Some(message) = state_lock.get_message(self.message)? else {
            return Ok(GenericResponse(Status::NotFound));
        };
        if Some(message.author_uuid) != peer.uuid {
            return Ok(GenericResponse(Status::Forbidden));
        }

        state_lock.edit_message(message.uuid, self.new_content.as_str())?;

        let msg = Response::MessageEditedResponse {
            message: self.message,
            new_content: self.new_content.clone(),
        };

        let mut msg_json = serde_json::to_value(msg)?;
        msg_json["status"] = (Status::Ok as i32).into();
        state_lock.send_to_all(msg_json)?;

        Ok(GenericResponse(Status::Ok))
    }
}

impl Request for DeleteRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }
        let Some(message) = state_lock.get_message(self.message)? else {
            return Ok(GenericResponse(Status::NotFound));
        };
        if Some(message.author_uuid) != peer.uuid {
            return Ok(GenericResponse(Status::Forbidden));
        }

        state_lock.delete_message(self.message)?;

        let msg = Response::MessageDeletedResponse {
            message: self.message,
        };

        let mut msg_json = serde_json::to_value(msg)?;
        msg_json["status"] = (Status::Ok as i32).into();
        state_lock.send_to_all(msg_json)?;

        Ok(GenericResponse(Status::Ok))
    }
}

impl Request for NickRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }

        // do not allow registering a duplicate username
        if state_lock
            .get_user_by_name(&self.nick)
            .is_ok_and(|x| x.is_some())
        {
            return Ok(GenericResponse(Status::Conflict));
        }

        let Some(mut user) = state_lock.get_user(&peer.uuid.unwrap())? else {
            return Ok(GenericResponse(Status::NotFound));
        };

        user.name = self.nick.to_string();

        state_lock.update_user(&user)?;
        send_metadata(state_lock, peer);
        Ok(GenericResponse(Status::Ok))
    }
}

impl Request for OnlineRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }

        Ok(OnlineResponse {
            data: state_lock.online.keys().copied().collect(),
        })
    }
}

impl Request for SendRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }
        // Check for an empty message, or one that contains only whitespace
        if self.content.chars().all(|c| c.is_whitespace()) {
            return Ok(GenericResponse(Status::BadRequest));
        }

        // check that we're sending to a channel that exists
        if !state_lock.channel_exists(&self.channel)? {
            return Ok(GenericResponse(Status::NotFound));
        }

        // check if we're replying to a message that it exists
        if let Some(r) = self.reply {
            let exists = state_lock.message_exists(&r)?;
            if !exists {
                return Ok(GenericResponse(Status::NotFound));
            }
        }

        let msg = Message {
            uuid: gen_uuid(),
            content: self.content.to_owned(),
            author_uuid: peer.uuid.unwrap(),
            channel_uuid: self.channel,
            date: chrono::offset::Utc::now().timestamp() as i32,
            edited: false,
            reply: self.reply,
        };
        state_lock.add_to_history(&msg)?;

        let uuid = msg.uuid; // save for later

        let response = ContentResponse {
            message: msg.into(),
        };
        let mut msg_json = serde_json::to_value(response)?;
        msg_json["status"] = (Status::Ok as i32).into();
        state_lock.send_to_all(msg_json)?;
        Ok(SendResponse { message: uuid })
    }
}

impl Request for HistoryRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }
        if state_lock.get_channel(&self.channel)?.is_none() {
            return Ok(GenericResponse(Status::NotFound));
        }

        // history request may fail if before_message is not found. If that happens, catch this and return 404 not 500.
        let mut history = match state_lock.get_history(self.channel, self.num, self.before_message)
        {
            Ok(history) => history,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Ok(GenericResponse(Status::NotFound))
            }
            Err(e) => return Err(e.into()),
        };
        history.reverse();

        // simulate some lag
        // std::thread::sleep(std::time::Duration::from_secs(2));
        Ok(HistoryResponse { data: history })
    }
}

impl Request for PfpRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }

        // disallow profile pictures over 40kb, for now
        if self.data.len() > 40 * 1024 {
            return Ok(GenericResponse(Status::BadRequest));
        }

        match state_lock.get_user(&peer.uuid.unwrap())? {
            Some(mut user) => {
                user.pfp = self.data.to_string();

                state_lock.update_user(&user)?;
                send_metadata(state_lock, peer);
                Ok(GenericResponse(Status::Ok))
            }

            // TODO this should probably be an internal error, this user really should exist
            None => Ok(GenericResponse(Status::NotFound)),
        }
    }
}

impl Request for SyncSetRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }

        let mut sync_data = match state_lock.get_sync_data(&peer.uuid.unwrap())? {
            Some(data) => data,
            None => {
                let data = SyncData::new(peer.uuid.unwrap());
                state_lock.insert_sync_data(&data)?;
                data
            }
        };

        sync_data.uname.clone_from(&self.uname);
        sync_data.pfp.clone_from(&self.pfp);

        state_lock.update_sync_data(sync_data)?;

        Ok(GenericResponse(Status::Ok))
    }
}

impl Request for SyncGetRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }

        let sync_data = state_lock.get_sync_data(&peer.uuid.unwrap())?;
        match sync_data {
            Some(sync_data) => Ok(SyncGetResponse { data: sync_data }),
            None => Ok(GenericResponse(Status::NotFound)),
        }
    }
}
impl Request for SyncSetServersRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }

        state_lock.clear_sync_servers_of(peer.uuid.unwrap())?;

        for (idx, sync_server) in self.servers.iter().enumerate() {
            let mut server = sync_server.clone();
            server.user_uuid = peer.uuid.unwrap();
            server.idx = idx as i32;
            state_lock.insert_sync_server(server)?;
        }

        Ok(GenericResponse(Status::Ok))
    }
}

impl Request for SyncGetServersRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if !peer.logged_in() {
            return Ok(GenericResponse(Status::Forbidden));
        }
        let servers = state_lock.get_sync_servers(peer.uuid.unwrap())?;

        Ok(SyncGetServersResponse { servers })
    }
}
