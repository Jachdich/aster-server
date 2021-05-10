extern crate num;
#[macro_use]
extern crate num_derive;

#[derive(FromPrimitive)]
pub enum Permission {
    ModifyChannel,
    ChangeNick,
    DeleteMessage,
    EditMessage,
    EditGroups,
    EditUserGroups,
    SendMessages,
    JoinVoice,
    Root,
    ModifyServer,
}

pub fn has_perm(perms: i64, perm: Permission) -> bool {
    (perms >> (perm as u8)) & 0b1
}
