pub struct SyncServer {
    pub name: String,
    pub pfp_b64: String,
    pub user_uuid: u64,
}

pub struct SyncUser {
    pub uname: String,
    pub pfp_b64: String,
    pub servers: Vec<SyncServer>,
}
