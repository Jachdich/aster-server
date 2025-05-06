use crate::commands::{
    CmdError, Request,
    Response::{self, *},
    Status,
};
use crate::helper::LockedState;
use crate::Peer;
use crate::CONF;
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
    fn execute(self, state_lock: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(Response::GetMetadataResponse {
            data: state_lock.get_users()?,
        })
    }
}

impl Request for GetUserRequest {
    fn execute(self, state_lock: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        match state_lock.get_user(self.uuid)? {
            Some(peer_meta) => Ok(GetUserResponse { data: peer_meta }),
            None => Ok(GenericResponse(Status::NotFound)),
        }
    }
}

impl Request for GetIconRequest {
    fn execute(self, _: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(GetIconResponse {
            data: CONF.icon.to_owned(),
        })
    }
}

impl Request for GetNameRequest {
    fn execute(self, _: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(GetNameResponse {
            data: CONF.name.to_owned(),
        })
    }
}

impl Request for ListChannelsRequest {
    fn execute(self, state_lock: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        let channels = state_lock.get_channels()?;
        Ok(ListChannelsResponse { data: channels })
    }
}

impl Request for GetEmojiRequest {
    fn execute(self, state_lock: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        let data = state_lock.get_emoji(self.uuid)?;
        if let Some(data) = data {
            Ok(GetEmojiResponse { data })
        } else {
            Ok(GenericResponse(Status::NotFound))
        }
    }
}

impl Request for ListEmojiRequest {
    fn execute(self, state_lock: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(ListEmojiResponse {
            data: state_lock.list_emoji()?,
        })
    }
}

impl Request for LeaveRequest {
    fn execute(self, _: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(GenericResponse(Status::Ok))
    }
}

impl Request for PingRequest {
    fn execute(self, _: &mut LockedState, _: &mut Peer) -> Result<Response, CmdError> {
        Ok(GenericResponse(Status::Ok))
    }
}
