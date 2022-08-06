use crate::sharedchannel::SharedChannel;
use crate::schema;
use std::collections::HashMap;
use diesel::prelude::*;
use crate::models::*;
use crate::message::*;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use crate::peer::Pontoon;
use crate::CONF;

pub struct Shared {
    pub online: Vec<i64>,
    pub conn: SqliteConnection,
    pub peers: Vec<Pontoon>,
}

impl Shared {
    pub fn new() -> Self {
        let sqlitedb = SqliteConnection::establish(&CONF.database_file).expect(&format!("Fatal(Shared::new) connecting to the database file {}", &CONF.database_file));

        Shared {
            online: Vec::new(),
            conn: sqlitedb,
            peers: Vec::new(),
        }
    }

    pub fn load(&mut self) {
        let channels = self.get_channels();
        if channels.len() == 0 {
            let new_channel = Channel::new("general");
            self.insert_channel(new_channel);
        }
    }

    pub fn send_to_all(&self, message: MessageType) {
    	for peer in self.peers.iter() {
            peer.tx.send(&message);
    	}
    }

    //TODO WHAT??
    // fn save(&mut self) {
        // 
    // }

    pub fn get_user_by_name(&self, name: &str) -> Option<User> {
        let mut query_res = schema::users::table
                        .filter(schema::users::name.eq(name))
                        .limit(1)
                        .load::<User>(&self.conn).ok()?;
        query_res.pop()
    }

    pub fn get_users(&self) -> Vec<User> {
        return schema::users::table.load::<User>(&self.conn).unwrap();
    }

    pub fn get_channels(&self) -> Vec<Channel> {
        return schema::channels::table.load::<Channel>(&self.conn).unwrap();
    }

    pub fn get_user(&self, user: &i64) -> User {
        let mut results = schema::users::table
            .filter(schema::users::uuid.eq(user))
            .limit(1)
            .load::<User>(&self.conn)
            .expect("User does not exist");

        return results.remove(0);
    }

    //pub fn get_password(&self, user: &i64) -> 

    pub fn add_to_history(&self, msg: CookedMessage) {
        let new_msg = CookedMessageInsertable::new(msg);
        let _ = diesel::insert_into(schema::messages::table)
            .values(&new_msg)
            .execute(conn)
            .expect("Error appending to history");
    }

    pub fn get_channel(&self, channel: &i64) -> Channel {
        let mut results = schema::channels::table
            .filter(schema::channels::uuid.eq(channel))
            .limit(1)
            .load::<Channel>(&self.conn)
            .expect("Channel does not exist");

        return results.remove(0);
    }

    pub fn get_sync_data(&self, uuid: &i64) -> Option<SyncData> {
        let mut results = schema::sync_data::table
            .filter(schema::sync_data::user_uuid.eq(uuid))
            .limit(1)
            .load::<SyncData>(&self.conn)
            .expect(&format!("User '{}' does not have sync data", uuid));
        if results.len() > 0 {
            Some(results.remove(0))
        } else {
            None
        }
    }

    pub fn get_channel_by_name(&self, channel: &String) -> Result<Channel, diesel::result::Error> {
        let mut results = schema::channels::table
            .filter(schema::channels::name.eq(channel))
            .limit(1)
            .load::<Channel>(&self.conn)?;

        return Ok(results.remove(0));
    }

    pub fn insert_channel(&self, channel: Channel) -> Result<usize, diesel::result::Error> {
        diesel::insert_into(schema::channels::table).values(&channel).execute(&self.conn)
    }

    pub fn insert_user(&self, user: User) -> Result<usize, diesel::result::Error> {
        diesel::insert_into(schema::users::table).values(&user).execute(&self.conn)
    }

    pub fn insert_sync_data(&self, data: &SyncData) -> Result<usize, diesel::result::Error> {
        diesel::insert_into(schema::sync_data::table).values(data).execute(&self.conn)
    }

    pub fn insert_sync_server(&self, data: SyncServer) -> Result<usize, diesel::result::Error> {
        diesel::insert_into(schema::sync_servers::table).values(data).execute(&self.conn)
    }

    pub fn update_user(&self, user: User) -> Result<usize, diesel::result::Error> {
        diesel::update(schema::users::table.find(user.uuid))
            .set((schema::users::name.eq(user.name),
                  schema::users::pfp.eq(user.pfp),
                  schema::users::group_uuid.eq(user.group_uuid)))
            .execute(&self.conn)
    }

    pub fn update_sync_data(&self, data: SyncData) -> Result<usize, diesel::result::Error> {
        diesel::update(schema::sync_data::table.find(data.user_uuid))
            .set((schema::sync_data::uname.eq(data.uname),
                  schema::sync_data::pfp.eq(data.pfp)))
            .execute(&self.conn)
    }
}
