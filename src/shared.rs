use crate::helper::Uuid;
use crate::message::*;
use crate::models::*;
use crate::schema;
use crate::CONF;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
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
        todo!("Check the schema!!");
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

        smt.query_row([name], |row| {
            Ok(User {
                uuid: row.get(0)?,
                name: row.get(1)?,
                pfp: row.get(2)?,
                group_uuid: row.get(3)?,
                password: row.get(4)?,
            })
        })
        .optional()
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
        Ok(!self
            .conn
            .prepare("select exists(select 1 from messages where uuid=?1)")?
            .query_row([uuid], |row| Ok(row.get::<usize, i32>(0)? == 1))?)
    }

    pub fn get_user(&mut self, user: &i64) -> Result<Option<User>, DbError> {
        self.conn
            .prepare("select * from users where uuid = ?1")?
            .query_row([user], |row| {
                Ok(User {
                    uuid: row.get(0)?,
                    name: row.get(1)?,
                    pfp: row.get(2)?,
                    group_uuid: row.get(3)?,
                    password: row.get(4)?,
                })
            })
            .optional()
    }

    pub fn add_to_history(&mut self, msg: &Message) -> Result<(), DbError> {
        // let _ = diesel::insert_into(schema::messages::table)
        //     .values(msg)
        //     .execute(&mut self.conn)?;
        // Ok(())
        //         uuid -> BigInt,
        //         content -> Text,
        //         author_uuid -> BigInt,
        //         channel_uuid -> BigInt,
        //         date -> Integer,
        //         edited -> Bool,
        //         reply -> Nullable<BigInt>,
        //         rowid -> Integer,
        self.conn
            .prepare("insert into messages values (?1, ?2, ?3, ?4, ?5)")?
            .execute(rusqlite::params![
                msg.uuid,
                &msg.content,
                msg.author_uuid,
                msg.channel_uuid,
                msg.date,
                msg.edited,
                msg.reply,
            ])?;
        Ok(())
    }

    pub fn get_channel(&mut self, channel: &Uuid) -> Result<Option<Channel>, DbError> {
        // let mut results = schema::channels::table
        //     .filter(schema::channels::uuid.eq(channel))
        //     .limit(1)
        //     .load::<Channel>(&mut self.conn)?;

        // if results.len() == 1 {
        //     Ok(Some(results.remove(0)))
        // } else {
        //     Ok(None)
        // }
        self.conn
            .prepare("select * from channels where uuid = ?1")?
            .query_row([channel], |row| {
                Ok(Channel {
                    uuid: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .optional()
    }

    pub fn get_message(&mut self, message: Uuid) -> Result<Option<Message>, DbError> {
        // let mut results = schema::messages::table
        //     .filter(schema::messages::uuid.eq(&message))
        //     .limit(1)
        //     .load::<Message>(&mut self.conn)?;
        // if results.len() == 1 {
        //     Ok(Some(results.remove(0)))
        // } else {
        //     Ok(None)
        // }
        self.conn
            .prepare("select * from messages where uuid = ?1 limit 1")?
            .query_row([message], |row| {
                Ok(Message {
                    uuid: row.get(0)?,
                    content: row.get(1)?,
                    author_uuid: row.get(2)?,
                    channel_uuid: row.get(3)?,
                    date: row.get(4)?,
                    edited: row.get(5)?,
                    reply: row.get(6)?,
                    rowid: row.get(7)?,
                })
            })
            .optional()
    }

    pub fn get_sync_data(&mut self, uuid: &Uuid) -> Result<Option<SyncData>, DbError> {
        // let mut results = schema::sync_data::table
        //     .filter(schema::sync_data::user_uuid.eq(uuid))
        //     .limit(1)
        //     .load::<SyncData>(&mut self.conn)?;
        // if !results.is_empty() {
        //     Ok(Some(results.remove(0)))
        // } else {
        //     Ok(None)
        // }
        self.conn
            .prepare("select * from sync_data where user_uuid = ?1 limit 1")?
            .query_row([uuid], |row| {
                Ok(SyncData {
                    user_uuid: row.get(0)?,
                    uname: row.get(1)?,
                    pfp: row.get(2)?,
                })
            })
            .optional()
    }

    pub fn get_channel_by_name(&mut self, channel: &str) -> Result<Option<Channel>, DbError> {
        // let mut results = schema::channels::table
        //     .filter(schema::channels::name.eq(channel))
        //     .limit(1)
        //     .load::<Channel>(&mut self.conn)?;

        // if results.len() == 1 {
        //     Ok(Some(results.remove(0)))
        // } else {
        //     Ok(None)
        // }
        self.conn
            .prepare("select * from channels where name = ?1")?
            .query_row([channel], |row| {
                Ok(Channel {
                    uuid: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .optional()
    }

    pub fn insert_channel(&mut self, channel: Channel) -> Result<usize, DbError> {
        // diesel::insert_into(schema::channels::table)
        //     .values(&channel)
        //     .execute(&mut self.conn)
        self.conn
            .prepare("insert into channels values (?1, ?2)")?
            .execute(params![channel.uuid, channel.name])
    }

    pub fn insert_user(&mut self, user: User) -> Result<usize, DbError> {
        // diesel::insert_into(schema::users::table)
        //     .values(&user)
        //     .execute(&mut self.conn)
        self.conn
            .prepare("insert into users values (?1, ?2, ?3, ?4, ?5)")?
            .execute(params![
                user.uuid,
                user.name,
                user.pfp,
                user.group_uuid,
                user.password
            ])
    }

    pub fn insert_sync_data(&mut self, data: &SyncData) -> Result<usize, DbError> {
        //     diesel::insert_into(schema::sync_data::table)
        //         .values(data)
        //         .execute(&mut self.conn)
        self.conn
            .prepare("insert into sync_data values (?1, ?2, ?3)")?
            .execute(params![data.user_uuid, data.uname, data.pfp])
    }

    pub fn insert_sync_server(&mut self, data: SyncServer) -> Result<usize, DbError> {
        // diesel::insert_into(schema::sync_servers::table)
        //     .values(data)
        //     .execute(&mut self.conn)
        self.conn
            .prepare("insert into sync_servers values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")?
            .execute(params![
                data.user_uuid,
                data.uuid,
                data.uname,
                data.ip,
                data.port,
                data.pfp,
                data.name,
                data.idx,
            ])
    }

    pub fn update_user(&mut self, user: &User) -> Result<usize, DbError> {
        // diesel::update(schema::users::table.find(user.uuid))
        //     .set((
        //         schema::users::name.eq(&user.name),
        //         schema::users::pfp.eq(&user.pfp),
        //         schema::users::group_uuid.eq(user.group_uuid),
        //         schema::users::password.eq(&user.password),
        //     ))
        //     .execute(&mut self.conn)
        self.conn.prepare("update idk how update statements work")
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
