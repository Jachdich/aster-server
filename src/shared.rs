use crate::message::*;
use crate::models::*;
use crate::peer::Pontoon;
use crate::schema;
use crate::CONF;
use diesel::prelude::*;
use std::collections::HashMap;

pub struct Shared {
    pub online: HashMap<i64, u32>,
    pub conn: SqliteConnection,
    pub peers: Vec<Pontoon>,
}

impl Shared {
    pub fn new() -> Self {
        let sqlitedb = SqliteConnection::establish(&CONF.database_file).unwrap_or_else(|_| panic!(
            "Fatal(Shared::new) connecting to the database file {}",
            &CONF.database_file
        ));

        Shared {
            online: HashMap::new(),
            conn: sqlitedb,
            peers: Vec::new(),
        }
    }

    pub fn load(&mut self) {
        let channels = self.get_channels();
        if channels.is_empty() {
            let new_channel = Channel::new("general");
            self.insert_channel(new_channel)
                .expect("Fatal(Shared::load): couldn't insert channel, broken database?");
        }
    }

    pub fn send_to_all(&self, message: serde_json::Value) {
        for peer in self.peers.iter() {
            if let Err(e) = peer.tx.send(message.clone()) {
                println!("Error(Shared::send_to_all): I think this is unlikely but `peer.tx.send` failed. idk bug me to fix it ig. {:?}", e);
            }
        }
    }

    pub fn inc_online(&mut self, user: i64) {
        let orig_count = match self.online.get(&user) {
            Some(count) => *count,
            None => 0,
        };
        self.online.insert(user, orig_count + 1);
    }

    //TODO WHAT??
    // fn save(&mut self) {
    //
    // }

    pub fn get_user_by_name(&mut self, name: &str) -> Option<User> {
        let mut query_res = schema::users::table
            .filter(schema::users::name.eq(name))
            .limit(1)
            .load::<User>(&mut self.conn)
            .ok()?;
        query_res.pop()
    }

    pub fn get_users(&mut self) -> Vec<User> {
        schema::users::table.load::<User>(&mut self.conn).unwrap()
    }

    pub fn get_channels(&mut self) -> Vec<Channel> {
        schema::channels::table.load::<Channel>(&mut self.conn).unwrap()
    }

    pub fn get_user(&mut self, user: &i64) -> Result<Option<User>, diesel::result::Error> {
        let mut results = schema::users::table
            .filter(schema::users::uuid.eq(user))
            .limit(1)
            .load::<User>(&mut self.conn)?;

        if results.len() == 1 {
            Ok(Some(results.remove(0)))
        } else {
            Ok(None)
        }
    }

    //pub fn get_password(&self, user: &i64) ->

    pub fn add_to_history(&mut self, msg: &Message) {
        let _ = diesel::insert_into(schema::messages::table)
            .values(msg)
            .execute(&mut self.conn)
            .expect("Error appending to history");
    }

    pub fn get_channel(&mut self, channel: &i64) -> Result<Option<Channel>, diesel::result::Error> {
        let mut results = schema::channels::table
            .filter(schema::channels::uuid.eq(channel))
            .limit(1)
            .load::<Channel>(&mut self.conn)?;

        if results.len() == 1 {
            Ok(Some(results.remove(0)))
        } else {
            Ok(None)
        }
    }

    pub fn get_sync_data(&mut self, uuid: &i64) -> Result<Option<SyncData>, diesel::result::Error> {
        let mut results = schema::sync_data::table
            .filter(schema::sync_data::user_uuid.eq(uuid))
            .limit(1)
            .load::<SyncData>(&mut self.conn)?;
        if !results.is_empty() {
            Ok(Some(results.remove(0)))
        } else {
            Ok(None)
        }
    }

    pub fn get_channel_by_name(&mut self, channel: &String) -> Result<Option<Channel>, diesel::result::Error> {
        let mut results = schema::channels::table
            .filter(schema::channels::name.eq(channel))
            .limit(1)
            .load::<Channel>(&mut self.conn)?;

        if results.len() == 1 {
            Ok(Some(results.remove(0)))
        } else {
            Ok(None)
        }
    }

    pub fn insert_channel(&mut self, channel: Channel) -> Result<usize, diesel::result::Error> {
        diesel::insert_into(schema::channels::table)
            .values(&channel)
            .execute(&mut self.conn)
    }

    pub fn insert_user(&mut self, user: User) -> Result<usize, diesel::result::Error> {
        diesel::insert_into(schema::users::table)
            .values(&user)
            .execute(&mut self.conn)
    }

    pub fn insert_sync_data(&mut self, data: &SyncData) -> Result<usize, diesel::result::Error> {
        diesel::insert_into(schema::sync_data::table)
            .values(data)
            .execute(&mut self.conn)
    }

    pub fn insert_sync_server(&mut self, data: SyncServer) -> Result<usize, diesel::result::Error> {
        diesel::insert_into(schema::sync_servers::table)
            .values(data)
            .execute(&mut self.conn)
    }

    pub fn update_user(&mut self, user: User) -> Result<usize, diesel::result::Error> {
        diesel::update(schema::users::table.find(user.uuid))
            .set((
                schema::users::name.eq(user.name),
                schema::users::pfp.eq(user.pfp),
                schema::users::group_uuid.eq(user.group_uuid),
            ))
            .execute(&mut self.conn)
    }

    pub fn update_sync_data(&mut self, data: SyncData) -> Result<usize, diesel::result::Error> {
        diesel::update(schema::sync_data::table.find(data.user_uuid))
            .set((
                schema::sync_data::uname.eq(data.uname),
                schema::sync_data::pfp.eq(data.pfp),
            ))
            .execute(&mut self.conn)
    }
}
