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

/// Create a new account with the given username and password. Returns a packet of type register with a field "uuid"
/// containing the uuid of the newly created account.  
/// Error conditions:
/// - 409 (conflict) if the username already exists within the server.
/// - 400 (bad request) if the username is empty or entirely whitespace.
/// - 405 (method not allowed) if already logged in.
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

        let user = User {
            name: self.uname,
            pfp: CONF.default_pfp.to_owned(),
            uuid: gen_uuid(),
            password: make_hash(&self.passwd)?,
            groups: Vec::new(),
        };

        state_lock.insert_user(&user)?;
        peer.uuid = Some(user.uuid);

        // stoopid
        for p in &mut state_lock.peers {
            if p.1 == peer.addr {
                p.2 = Some(user.uuid);
            }
        }

        state_lock.inc_online(user.uuid);

        send_metadata(state_lock, peer);
        send_online(state_lock);

        Ok(RegisterResponse { uuid: user.uuid })
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
            state_lock.get_user(uuid)?
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
        // stoopid: the sequel
        // (actually this just makes sure that the shared's peers list has the right uuid)
        for p in &mut state_lock.peers {
            if p.1 == peer.addr {
                p.2 = Some(user.uuid);
            }
        }

        state_lock.inc_online(user.uuid);
        send_metadata(state_lock, peer);
        send_online(state_lock);
        Ok(LoginResponse { uuid: user.uuid })
    }
}
