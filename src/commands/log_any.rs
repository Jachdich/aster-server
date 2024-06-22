use crate::commands::{
    CmdError, Request,
    Response::{self, *},
    Status,
};
use crate::helper::LockedState;
use crate::models::Emoji;
use crate::schema;
use crate::Peer;
use crate::CONF;
use diesel::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct GetIconRequest;
#[derive(Deserialize)]
pub struct GetNameRequest;
#[derive(Deserialize)]
pub struct GetMetadataRequest;
#[derive(Deserialize)]
pub struct ListChannelsRequest;
#[derive(Deserialize)]
pub struct ListEmojiRequest;
#[derive(Deserialize)]
pub struct LeaveRequest;
#[derive(Deserialize)]
pub struct PingRequest;

#[derive(Deserialize)]
pub struct GetEmojiRequest {
    pub uuid: i64,
}
#[derive(Deserialize)]
pub struct GetUserRequest {
    pub uuid: i64,
}

impl Request for GetMetadataRequest {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(Response::GetMetadataResponse {
            data: state_lock.get_users()?,
        })
    }
}

impl Request for GetUserRequest {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        match state_lock.get_user(&self.uuid)? {
            Some(peer_meta) => Ok(GetUserResponse { data: peer_meta }),
            None => Ok(GenericResponse(Status::NotFound)),
        }
    }
}

impl Request for GetIconRequest {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(GetIconResponse {
            data: CONF.icon.to_owned(),
        })
    }
}

impl Request for GetNameRequest {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(GetNameResponse {
            data: CONF.name.to_owned(),
        })
    }
}

impl Request for ListChannelsRequest {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        let channels = state_lock.get_channels()?;
        Ok(ListChannelsResponse { data: channels })
    }
}

impl Request for GetEmojiRequest {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        let mut results = schema::emojis::table
            .filter(schema::emojis::uuid.eq(self.uuid))
            .limit(1)
            .load::<Emoji>(&mut state_lock.conn)
            .unwrap();
        if results.is_empty() {
            Ok(GenericResponse(Status::NotFound))
        } else {
            Ok(GetEmojiResponse {
                data: results.remove(0),
            })
        }
    }
}

impl Request for ListEmojiRequest {
    fn execute(&self, state_lock: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        let results = schema::emojis::table
            .load::<Emoji>(&mut state_lock.conn)
            .unwrap();
        Ok(ListEmojiResponse {
            data: results
                .iter()
                .map(|res| (res.name.clone(), res.uuid))
                .collect::<Vec<(String, i64)>>(),
        })
    }
}

impl Request for LeaveRequest {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(GenericResponse(Status::Ok))
    }
}

impl Request for PingRequest {
    fn execute(&self, _: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(GenericResponse(Status::Ok))
    }
}
