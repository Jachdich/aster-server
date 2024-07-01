use crate::commands::{
    send_metadata, send_online, CmdError,
    Response::{self, *},
};
use crate::commands::{Request, Status};
use crate::helper::{gen_uuid, LockedState};
use crate::models::User;
use crate::Peer;
use crate::CONF;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub passwd: String,
    pub uname: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub passwd: String,
    pub uname: Option<String>,
    pub uuid: Option<i64>,
}

impl Request for RegisterRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if peer.logged_in() {
            //registering doesn't make sense when logged in
            return Ok(GenericResponse(Status::MethodNotAllowed));
        }

        // do not allow registering a duplicate username
        if state_lock.get_user_by_name(&self.uname).is_ok_and(|x| x.is_some()) {
            return Ok(GenericResponse(Status::Conflict));
        }

        let uuid = gen_uuid();
        let user = User {
            name: self.uname.to_owned(),
            pfp: CONF.default_pfp.to_owned(),
            uuid,
            group_uuid: 0,
        };

        state_lock.insert_user(user)?;
        peer.uuid = Some(uuid);

        state_lock.inc_online(uuid);

        send_metadata(state_lock, peer);
        send_online(state_lock);

        Ok(RegisterResponse { uuid })
    }
}

impl Request for LoginRequest {
    fn execute(&self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if peer.logged_in() {
            //logging in doesn't make sense when already logged in
            return Ok(GenericResponse(Status::MethodNotAllowed));
        }

        let uuid = if let Some(uname) = &self.uname {
            if let Some(user) = state_lock.get_user_by_name(uname)? {
                user.uuid
            } else {
                return Ok(GenericResponse(Status::NotFound));
            }
        } else if let Some(uuid) = self.uuid {
            uuid
        } else {
            //neither uname nor uuid were provided
            return Ok(GenericResponse(Status::BadRequest));
        };

        // check the uuid exists
        if state_lock.get_user(&uuid)?.is_none() {
            return Ok(GenericResponse(Status::NotFound));
        }

        //TODO confirm password
        peer.uuid = Some(uuid);

        state_lock.inc_online(uuid);
        send_metadata(state_lock, peer);
        send_online(state_lock);
        Ok(LoginResponse { uuid })
    }
}
