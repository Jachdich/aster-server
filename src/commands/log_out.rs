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

use super::auth::{check_password, make_hash};

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
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if peer.logged_in() {
            //registering doesn't make sense when logged in
            return Ok(GenericResponse(Status::MethodNotAllowed));
        }

        // do not allow empty usernames
        if self.uname.trim().is_empty() {
            return Ok(GenericResponse(Status::BadRequest));
        }

        // do not allow registering a duplicate username
        if state_lock
            .get_user_by_name(&self.uname)
            .is_ok_and(|x| x.is_some())
        {
            return Ok(GenericResponse(Status::Conflict));
        }

        let uuid = gen_uuid();
        let user = User {
            name: self.uname,
            pfp: CONF.default_pfp.to_owned(),
            uuid,
            group_uuid: 0,
            password: make_hash(&self.passwd)?,
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
    fn execute(self, state_lock: &mut LockedState, peer: &mut Peer) -> Result<Response, CmdError> {
        if peer.logged_in() {
            //logging in doesn't make sense when already logged in
            return Ok(GenericResponse(Status::MethodNotAllowed));
        }

        let user = if let Some(uname) = &self.uname {
            state_lock.get_user_by_name(uname)?
        } else if let Some(uuid) = self.uuid {
            state_lock.get_user(&uuid)?
        } else {
            //neither uname nor uuid were provided
            return Ok(GenericResponse(Status::BadRequest));
        };

        // check the user exists
        let Some(mut user) = user else {
            return Ok(GenericResponse(Status::NotFound));
        };

        // TODO temporarily allow users without passwords to log in
        if user.password.is_empty() {
            user.password = make_hash(&self.passwd)?;
            state_lock.update_user(&user)?;
        } else {
            if !check_password(&self.passwd, &user.password)? {
                return Ok(GenericResponse(Status::Forbidden));
            }
        }

        peer.uuid = Some(user.uuid);

        state_lock.inc_online(user.uuid);
        send_metadata(state_lock, peer);
        send_online(state_lock);
        Ok(LoginResponse { uuid: user.uuid })
    }
}
