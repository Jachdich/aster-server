

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
    ((perms >> (perm as u8)) & 0b1) != 0
}

pub fn set_perm(perms: i64, perm: Permission) -> i64 {
    perms | (1 << perm as u8)
}

pub fn reset_perm(perms: i64, perm: Permission) -> i64 {
    perms & !(1 << perm as u8)
}
