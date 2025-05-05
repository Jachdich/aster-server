// See aster updates discord, pinned messages
pub struct Perm;

impl Perm {
    pub const MODIFY_CHANNEL: i64 = 1 << 0;
    pub const CHANGE_NICK: i64 = 1 << 1;
    pub const DELETE_MESSAGE: i64 = 1 << 2;
    pub const EDIT_MESSAGE: i64 = 1 << 3;
    pub const EDIT_GROUPS: i64 = 1 << 4;
    pub const EDIT_USER_GROUPS: i64 = 1 << 5;
    pub const SEND_MESSAGE: i64 = 1 << 6;
    pub const JOIN_VOICE: i64 = 1 << 7;
    pub const ROOT: i64 = 1 << 8;
    pub const MODFY_SERVER: i64 = 1 << 9;
}
