use crate::helper::Uuid;
use crate::message::*;
use crate::models::*;
use crate::schema;
use crate::CONF;
use rusqlite::Connection;
use std::collections::HashMap;
use tokio::sync::mpsc;

pub struct Shared {
    pub online: HashMap<i64, u32>,
    pub conn: Connection,
    pub peers: Vec<(
        mpsc::UnboundedSender<serde_json::Value>,
        std::net::SocketAddr,
    )>,
}

type DbError = rusqlite::Error;

impl Shared {
    pub fn new() -> Self {
        // let sqlitedb = SqliteConnection::establish(&CONF.database_file).unwrap_or_else(|_| {
        //     panic!(
        //         "Fatal(Shared::new) connecting to the database file {}",
        //         &CONF.database_file
        //     )
        // });
        let sqlitedb = Connection::open_in_memory().expect("Unable to create a DB?");

        Shared {
            online: HashMap::new(),
            conn: sqlitedb,
            peers: Vec::new(),
        }
    }

    pub fn load(&mut self) {
        let channels = self.get_channels().unwrap(); //TODO get rid of this unwrap
        if channels.is_empty() {
            let new_channel = Channel::new("general");
            self.insert_channel(new_channel)
                .expect("Fatal(Shared::load): couldn't insert channel, broken database?");
        }
    }

    pub fn send_to_all(
        &self,
        message: serde_json::Value,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<serde_json::Value>> {
        for (tx, _) in self.peers.iter() {
            tx.send(message.clone())?;
        }
        Ok(())
    }

    pub fn inc_online(&mut self, user: i64) {
        let orig_count = match self.online.get(&user) {
            Some(count) => *count,
            None => 0,
        };
        self.online.insert(user, orig_count + 1);
    }

    pub fn get_user_by_name(&mut self, name: &str) -> Result<Option<User>, DbError> {
        let mut smt = self
            .conn
            .prepare("SELECT * FROM users WHERE name = ?1 LIMIT 1")?;

        let res = smt
            .query_map([name], |row| {
                Ok(User {
                    uuid: row.get(0)?,
                    name: row.get(1)?,
                    pfp: row.get(2)?,
                    group_uuid: row.get(3)?,
                    password: row.get(4)?,
                })
            })?
            .next()
            .transpose(); // next returns Option<Result<...>>, however it makes more sense to return Result<Option<...>>
        res // don't question why res needs to be a variable...
    }

    pub fn get_users(&mut self) -> Result<Vec<User>, DbError> {
        self.conn
            .prepare("SELECT * FROM USERS")?
            .query_map([], |row| {
                Ok(User {
                    uuid: row.get(0)?,
                    name: row.get(1)?,
                    pfp: row.get(2)?,
                    group_uuid: row.get(3)?,
                    password: row.get(4)?,
                })
            })?
            .collect()
    }

    pub fn get_channels(&mut self) -> Result<Vec<Channel>, DbError> {
        self.conn
            .prepare("SELECT * FROM CHANNELS")?
            .query_map([], |row| {
                Ok(Channel {
                    uuid: row.get(0)?,
                    name: row.get(1)?,
                })
            })?
            .collect()
    }

    pub fn channel_exists(&mut self, uuid: &Uuid) -> Result<bool, DbError> {
        // TODO this might be slow
        Ok(self
            .get_channels()?
            .iter()
            .any(|channel| channel.uuid == *uuid))
    }
    pub fn message_exists(&mut self, uuid: &Uuid) -> Result<bool, DbError> {
        // TODO this might be slow
        Ok(!schema::messages::table
            .filter(schema::messages::uuid.eq(uuid))
            .limit(1)
            .load::<Message>(&mut self.conn)?
            .is_empty())
    }

    pub fn get_user(&mut self, user: &i64) -> Result<Option<User>, DbError> {
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

    pub fn add_to_history(&mut self, msg: &NewMessage) -> Result<(), DbError> {
        let _ = diesel::insert_into(schema::messages::table)
            .values(msg)
            .execute(&mut self.conn)?;
        Ok(())
    }

    pub fn get_channel(&mut self, channel: &i64) -> Result<Option<Channel>, DbError> {
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

    pub fn get_message(&mut self, message: Uuid) -> Result<Option<Message>, DbError> {
        let mut results = schema::messages::table
            .filter(schema::messages::uuid.eq(&message))
            .limit(1)
            .load::<Message>(&mut self.conn)?;
        if results.len() == 1 {
            Ok(Some(results.remove(0)))
        } else {
            Ok(None)
        }
    }

    pub fn get_sync_data(&mut self, uuid: &i64) -> Result<Option<SyncData>, DbError> {
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

    pub fn get_channel_by_name(&mut self, channel: &String) -> Result<Option<Channel>, DbError> {
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

    pub fn insert_channel(&mut self, channel: Channel) -> Result<usize, DbError> {
        diesel::insert_into(schema::channels::table)
            .values(&channel)
            .execute(&mut self.conn)
    }

    pub fn insert_user(&mut self, user: User) -> Result<usize, DbError> {
        diesel::insert_into(schema::users::table)
            .values(&user)
            .execute(&mut self.conn)
    }

    pub fn insert_sync_data(&mut self, data: &SyncData) -> Result<usize, DbError> {
        diesel::insert_into(schema::sync_data::table)
            .values(data)
            .execute(&mut self.conn)
    }

    pub fn insert_sync_server(&mut self, data: SyncServer) -> Result<usize, DbError> {
        diesel::insert_into(schema::sync_servers::table)
            .values(data)
            .execute(&mut self.conn)
    }

    pub fn update_user(&mut self, user: &User) -> Result<usize, DbError> {
        diesel::update(schema::users::table.find(user.uuid))
            .set((
                schema::users::name.eq(&user.name),
                schema::users::pfp.eq(&user.pfp),
                schema::users::group_uuid.eq(user.group_uuid),
                schema::users::password.eq(&user.password),
            ))
            .execute(&mut self.conn)
    }

    pub fn update_sync_data(&mut self, data: SyncData) -> Result<usize, DbError> {
        diesel::update(schema::sync_data::table.find(data.user_uuid))
            .set((
                schema::sync_data::uname.eq(data.uname),
                schema::sync_data::pfp.eq(data.pfp),
            ))
            .execute(&mut self.conn)
    }

    pub fn get_emoji(&mut self, uuid: Uuid) -> Result<Option<Emoji>, DbError> {
        let mut results = schema::emojis::table
            .filter(schema::emojis::uuid.eq(uuid))
            .limit(1)
            .load::<Emoji>(&mut self.conn)?;
        if results.is_empty() {
            Ok(None)
        } else {
            Ok(Some(results.remove(0)))
        }
    }

    pub fn list_emoji(&mut self) -> Result<Vec<(String, Uuid)>, DbError> {
        let results = schema::emojis::table.load::<Emoji>(&mut self.conn)?;
        Ok(results
            .into_iter()
            .map(|res| (res.name, res.uuid))
            .collect::<Vec<(String, Uuid)>>())
    }

    pub fn edit_message(&mut self, uuid: Uuid, new_content: &str) -> Result<usize, DbError> {
        diesel::update(schema::messages::table.filter(schema::messages::uuid.eq(uuid)))
            .set((
                schema::messages::content.eq(new_content),
                schema::messages::edited.eq(true),
            ))
            .execute(&mut self.conn)
    }

    pub fn delete_message(&mut self, uuid: Uuid) -> Result<usize, DbError> {
        diesel::delete(schema::messages::table.filter(schema::messages::uuid.eq(uuid)))
            .execute(&mut self.conn)
    }

    pub fn clear_sync_servers_of(&mut self, user: Uuid) -> Result<usize, DbError> {
        diesel::delete(schema::sync_servers::table.filter(schema::sync_servers::user_uuid.eq(user)))
            .execute(&mut self.conn)
    }

    pub fn get_sync_servers(&mut self, user: Uuid) -> Result<Vec<SyncServer>, DbError> {
        schema::sync_servers::table
            .filter(schema::sync_servers::user_uuid.eq(user))
            .order(schema::sync_servers::idx.asc())
            .load::<SyncServerQuery>(&mut self.conn)
            .map(|servers| {
                servers
                    .into_iter()
                    .map(SyncServer::from)
                    .collect::<Vec<SyncServer>>()
            })
    }
}
