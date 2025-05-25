use crate::helper::gen_uuid;
use crate::helper::Uuid;
use crate::message::*;
use crate::models::*;
use crate::permissions::Perm;
use crate::permissions::PermableEntity;
use crate::permissions::Permissions;
use crate::CONF;
use base64::engine::general_purpose;
use base64::Engine;
use rusqlite::ffi::SQLITE_CONFIG_PAGECACHE;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use std::collections::HashMap;
use std::io::Read;
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
const LATEST_VERSION: i32 = 4;

// TODO add unique constraints where applicable
fn latest_schema() -> String {
    format!(
        r#"
BEGIN;
CREATE TABLE version (
    version integer NOT NULL
);
INSERT INTO version VALUES({});
CREATE TABLE server_config (
    name text NOT NULL,
    pfp blob NOT NULL,
    base_perms blob NOT NULL,
);
CREATE TABLE channels (
    uuid BigInt PRIMARY KEY NOT NULL,
    name text NOT NULL,
    position Integer NOT NULL
);
INSERT INTO channels VALUES ({}, "general", 0);
CREATE TABLE messages (
    uuid BigInt PRIMARY KEY NOT NULL,
    content text NOT NULL,
    author_uuid BigInt NOT NULL,
    channel_uuid BigInt NOT NULL,
    date integer NOT NULL,
    edited Integer not null default 0,
    reply BigInt,
    FOREIGN KEY (author_uuid) REFERENCES users(uuid),
    FOREIGN KEY (channel_uuid) REFERENCES channels(uuid)
);
CREATE TABLE users (
    uuid BigInt PRIMARY KEY NOT NULL,
    name text NOT NULL,
    pfp text NOT NULL,
    group_uuid BigInt NOT NULL,
    password text NOT NULL,
    FOREIGN KEY (group_uuid) REFERENCES groups(uuid)
);
CREATE TABLE groups (
    uuid BigInt PRIMARY KEY NOT NULL,
    name text NOT NULL,
    colour integer NOT NULL,
    permissions Blob NOT NULL
);
CREATE TABLE user_groups (
    user_uuid BigInt NOT NULL,
    group_uuid BigInt NOT NULL,
    FOREIGN KEY (user_uuid) REFERENCES users(uuid),
    FOREIGN KEY (group_uuid) REFERENCES groups(uuid)
);
CREATE TABLE sync_data (
    user_uuid BigInt PRIMARY KEY NOT NULL,
    uname text NOT NULL,
    pfp text NOT NULL
);
CREATE TABLE emojis (
    uuid BigInt PRIMARY KEY NOT NULL,
    name text NOT NULL,
    data text NOT NULL
);
CREATE TABLE sync_servers (
    user_uuid BigInt NOT NULL,
    uuid BigInt,
    uname Text NOT NULL,
    ip Text NOT NULL,
    port Integer NOT NULL,
    pfp Text,
    name Text,
    idx Integer NOT NULL
);

CREATE TABLE channel_group_permissions (
    channel_uuid BigInt NOT NULL,
    user_uuid BigInt,
    group_uuid BigInt,
    permissions TODO TODO NOT NULL,
    FOREIGN KEY (user_uuid) REFERENCES users(uuid),
    FOREIGN KEY (group_uuid) REFERENCES groups(uuid)
);
COMMIT;"#,
        LATEST_VERSION,
        gen_uuid()
    )
}

struct Migration {
    from: i32,
    to: i32,
    sql: &'static str,
    f: Option<fn(&Connection) -> Result<(), DbError>>,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        from: 0,
        to: 1,
        sql:
            "begin; CREATE TABLE version (version integer NOT NULL); INSERT INTO version VALUES(0); commit;",
        f: None,
    },
    Migration {
        from: 1,
        to: 2,
        sql: r#"
            BEGIN;
            ALTER TABLE sync_servers RENAME TO sync_servers_2;
            CREATE TABLE sync_servers (
                user_uuid BigInt NOT NULL,
                uuid BigInt,
                uname Text NOT NULL,
                ip Text NOT NULL,
                port Integer NOT NULL,
                pfp Text,
                name Text,
                idx Integer NOT NULL
            );
            INSERT INTO sync_servers SELECT user_uuid,uuid,uname,ip,port,pfp,name,idx FROM sync_servers_2;
            DROP TABLE sync_servers_2;

            ALTER TABLE channels ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
            COMMIT;
        "#,
        f: Some(|sqlitedb: &Connection| {
            // order the channels by the order they currently appear.
            for (i, uuid) in sqlitedb
                .prepare("SELECT * FROM CHANNELS")?
                .query_map([], |row| row.get(0))?
                .enumerate()
            {
                let uuid: Uuid = uuid?;
                sqlitedb
                    .prepare("UPDATE channels SET position=?1 WHERE uuid=?2")?
                    .execute(params![i, uuid])?;
            }
            Ok(())
        }),
    },
    Migration {
        from: 2,
        to: 3,
        sql: r#"
            begin;
            CREATE TABLE channel_group_permissions (
                channel_uuid BigInt NOT NULL,
                user_uuid BigInt,
                group_uuid BigInt,
                permissions blob NOT NULL,
                FOREIGN KEY (user_uuid) REFERENCES users(uuid),
                FOREIGN KEY (group_uuid) REFERENCES groups(uuid)
            );

            drop table groups;
            CREATE TABLE groups (
                uuid BigInt PRIMARY KEY NOT NULL,
                permissions Blob NOT NULL,
                name text NOT NULL,
                colour integer NOT NULL
            );
            commit;
    "#,
        f: None,
    },

    Migration {
        from: 3, to: 4,
        sql: r#"
            begin;
            CREATE TABLE server_config (
                name text NOT NULL,
                pfp blob NOT NULL,
                base_perms blob NOT NULL,
            );
            commit;
        "#,

        f: Some(|sqlitedb: &Connection| {
            // TODO reuse code from init_tables
            let pfp_bytes = general_purpose::STANDARD.decode(&CONF.icon).unwrap();
            let default_base_perms = Permissions {
                modify_channels: Perm::Deny,
                modify_icon_name: Perm::Deny,
                modify_groups: Perm::Deny,
                modify_user_groups: Perm::Deny,
                ban_users: Perm::Deny,
                send_messages: Perm::Allow,
                read_messages: Perm::Allow,
                manage_messages: Perm::Deny,
                join_voice: Perm::Allow,
            };
            let perm_bytes: Box<[u8]> = default_base_perms.into();
            sqlitedb.execute("UPDATE server_config SET pfp = ?1, name = ?2, base_perms = ?3", params![pfp_bytes, &CONF.name, perm_bytes.into_vec()])?;
            Ok(())
        }),
    }
];

impl Shared {
    pub fn new(sqlitedb: Connection) -> Self {
        Shared {
            online: HashMap::new(),
            conn: sqlitedb,
            peers: Vec::new(),
        }
    }

    pub fn init_db(&self) {
        let version = self.get_db_version();
        let version = match version {
            Some(version) => version,
            None => {
                self.init_tables(&latest_schema());
                LATEST_VERSION
            }
        };

        self.apply_migrations(MIGRATIONS, version, LATEST_VERSION);
    }

    fn get_db_version(&self) -> Option<i32> {
        let table_exists = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type=?1 AND name=?2")
            .unwrap()
            .query_row(["table", "version"], |_| Ok(()))
            .optional()
            .unwrap()
            .is_some();
        if table_exists {
            let mut version_query = self
                .conn
                .prepare("SELECT * FROM version")
                .expect("Database failure to prepare version query");
            let version: Result<i32, DbError> = version_query.query_row([], |row| row.get(0));

            drop(version_query);

            Some(version.unwrap_or_else(|e| {
                panic!("Unable to read version for some other reason: {:?}", e)
            }))
        } else {
            // If we have a channels table, assume it's version 0, i.e. before the version table was added.
            let channels_exists = self
                .conn
                .prepare("SELECT name FROM sqlite_master WHERE type=?1 AND name=?2")
                .unwrap()
                .query_row(["table", "channels"], |_| Ok(()))
                .optional()
                .unwrap()
                .is_some();
            if channels_exists {
                Some(0)
            } else {
                None
            }
        }
    }

    fn init_tables(&self, init_sql: &str) {
        self.conn.execute_batch(init_sql).unwrap();
        let pfp_bytes = include_bytes!("../icon.png").to_vec();
        let default_base_perms = Permissions {
            modify_channels: Perm::Deny,
            modify_icon_name: Perm::Deny,
            modify_groups: Perm::Deny,
            modify_user_groups: Perm::Deny,
            ban_users: Perm::Deny,
            send_messages: Perm::Allow,
            read_messages: Perm::Allow,
            manage_messages: Perm::Deny,
            join_voice: Perm::Allow,
        };
        let perm_bytes: Box<[u8]> = default_base_perms.into();
        self.conn
            .execute(
                "UPDATE server_config SET pfp = ?1, name = ?2, base_perms = ?3",
                params![pfp_bytes, "Aster Server", perm_bytes.into_vec()],
            )
            .unwrap();
    }

    fn get_name(&self) -> Result<String, DbError> {}
    fn get_icon(&self) -> Result<Vec<u8>, DbError> {}
    fn get_base_perms(&self) -> Result<Permissions, DbError> {}
    fn set_name(&self) -> Result<String, DbError> {}
    fn set_icon(&self) -> Result<Vec<u8>, DbError> {}
    fn set_base_perms(&self) -> Result<Permissions, DbError> {}

    fn apply_migrations(
        &self,
        migrations: &[Migration],
        mut current_version: i32,
        latest_version: i32,
    ) {
        while current_version < latest_version {
            let applicable_migrations: Vec<_> = migrations
                .iter()
                .filter(|m| m.from == current_version)
                .collect();
            if applicable_migrations.len() == 0 {
                panic!(
                    "Unable to find a migration from db version {} (latest version is {})",
                    current_version, latest_version
                );
            }

            let m = applicable_migrations[0];
            log::info!("Applying migration from db version {} to {}", m.from, m.to);
            self.conn.execute_batch(m.sql).expect(&format!(
                "Failed to apply migration from db version {} to {}",
                m.from, m.to
            ));

            if let Some(f) = m.f {
                f(&self.conn).expect(&format!(
                    "Failed to run post-migration hook from db version {} to {}",
                    m.from, m.to
                ));
            }

            current_version = m.to;
            self.conn
                .execute_batch(&format!("UPDATE version SET version={};", current_version))
                .expect("Unable to update vesion in table");
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

    pub fn get_user_by_name(&self, name: &str) -> Result<Option<User>, DbError> {
        let mut smt = self
            .conn
            .prepare("SELECT * FROM users WHERE name = ?1 LIMIT 1")?;

        smt.query_row([name], |row| {
            let uuid = row.get(0)?;
            Ok(User {
                uuid,
                name: row.get(1)?,
                pfp: row.get(2)?,
                group_uuid: row.get(3)?,
                password: row.get(4)?,
                groups: self.get_group_uuids_of(uuid)?,
            })
        })
        .optional()
    }

    pub fn get_group(&self, group_uuid: Uuid) -> Result<Option<Group>, DbError> {
        self.conn
            .prepare("SELECT * FROM groups WHERE uuid = ?1")?
            .query_row([group_uuid], |row| {
                Ok(Group {
                    uuid: row.get(0)?,
                    permissions: row.get::<usize, Vec<u8>>(1)?.as_slice().into(),
                    name: row.get(2)?,
                    colour: row.get(3)?,
                })
            })
            .optional()
    }

    pub fn get_group_uuids_of(&self, user_uuid: Uuid) -> Result<Vec<Uuid>, DbError> {
        self.conn
            .prepare("SELECT * FROM user_groups WHERE user_uuid = ?1")?
            .query_map([user_uuid], |row| row.get(0))?
            .collect()
    }

    pub fn get_users(&self) -> Result<Vec<User>, DbError> {
        self.conn
            .prepare("SELECT * FROM USERS")?
            .query_map([], |row| {
                let uuid = row.get(0)?;
                Ok(User {
                    uuid,
                    name: row.get(1)?,
                    pfp: row.get(2)?,
                    group_uuid: row.get(3)?,
                    password: row.get(4)?,
                    groups: self.get_group_uuids_of(uuid)?,
                })
            })?
            .collect()
    }

    fn get_channel_permissions(
        &self,
        channel: Uuid,
    ) -> Result<HashMap<PermableEntity, Permissions>, DbError> {
        let mut map = HashMap::new();
        for (k, v) in self
            .conn
            .prepare("SELECT * FROM channel_group_permissions WHERE channel_uuid = ?1")?
            .query_map([channel], |row| {
                let group_uuid: Option<Uuid> = row.get(1)?;
                let user_uuid: Option<Uuid> = row.get(2)?;
                let perms: Vec<u8> = row.get(3)?;
                let perms: Permissions = perms.as_slice().into();
                // TODO doesn't handle both being Some
                if let Some(group_uuid) = group_uuid {
                    Ok((PermableEntity::Group(group_uuid), perms))
                } else if let Some(user_uuid) = user_uuid {
                    Ok((PermableEntity::User(user_uuid), perms))
                } else {
                    Err(DbError::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Null,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Both group_uuid and user_uuid are null",
                        )),
                    )) // TODO this is awful
                }
            })?
            .collect::<Result<Vec<(PermableEntity, Permissions)>, DbError>>()?
        {
            map.insert(k, v);
        }
        Ok(map)
    }

    pub fn get_channels(&self) -> Result<Vec<Channel>, DbError> {
        self.conn
            .prepare("SELECT * FROM CHANNELS")?
            .query_map([], |row| {
                let uuid = row.get(0)?;
                Ok(Channel {
                    uuid,
                    name: row.get(1)?,
                    position: row.get(2)?,
                    permissions: self.get_channel_permissions(uuid)?,
                })
            })?
            .collect()
    }

    pub fn channel_exists(&self, uuid: &Uuid) -> Result<bool, DbError> {
        // TODO this might be slow
        Ok(self
            .get_channels()?
            .iter()
            .any(|channel| channel.uuid == *uuid))
    }

    pub fn update_channel(&self, c: &Channel) -> Result<(), DbError> {
        self.conn
            .prepare("update channels set name = ?2, position = ?3 where uuid = ?1")?
            .execute(params![c.uuid, c.name, c.position])?;

        // again, probably not a good idea
        self.conn.execute(
            "delete from channel_group_permissions where channel_uuid = ?1",
            [c.uuid],
        )?;
        self.insert_channel_perms(c)
    }

    fn insert_channel_perms(&self, c: &Channel) -> Result<(), DbError> {
        let mut insert_query = self
            .conn
            .prepare("insert into channel_group_permissions values (?1, ?2, ?3, ?4)")?;
        for (k, v) in c.permissions.iter() {
            let (group_uuid, user_uuid) = match k {
                PermableEntity::User(user) => (None, Some(user)),
                PermableEntity::Group(group) => (Some(group), None),
            };
            let perms: Box<[u8]> = v.into();
            let perms = perms.into_vec();
            insert_query.execute(params![c.uuid, group_uuid, user_uuid, perms])?;
        }
        Ok(())
    }

    pub fn message_exists(&self, uuid: &Uuid) -> Result<bool, DbError> {
        Ok(self
            .conn
            .prepare("select exists(select 1 from messages where uuid=?1)")?
            .query_row([uuid], |row| Ok(row.get::<usize, i32>(0)? == 1))?)
    }

    pub fn get_user(&self, user: i64) -> Result<Option<User>, DbError> {
        self.conn
            .prepare("select * from users where uuid = ?1")?
            .query_row([user], |row| {
                let uuid = row.get(0)?;
                Ok(User {
                    uuid,
                    name: row.get(1)?,
                    pfp: row.get(2)?,
                    group_uuid: row.get(3)?,
                    password: row.get(4)?,
                    groups: self.get_group_uuids_of(uuid)?,
                })
            })
            .optional()
    }

    pub fn add_to_history(&self, msg: &Message) -> Result<(), DbError> {
        self.conn
            .prepare("insert into messages values (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?
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

    pub fn get_channel(&self, channel: &Uuid) -> Result<Option<Channel>, DbError> {
        self.conn
            .prepare("select * from channels where uuid = ?1")?
            .query_row([channel], |row| {
                let uuid = row.get(0)?;
                Ok(Channel {
                    uuid,
                    name: row.get(1)?,
                    position: row.get(2)?,
                    permissions: self.get_channel_permissions(uuid)?,
                })
            })
            .optional()
    }

    pub fn get_message(&self, message: Uuid) -> Result<Option<Message>, DbError> {
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
                })
            })
            .optional()
    }

    pub fn get_sync_data(&self, uuid: &Uuid) -> Result<Option<SyncData>, DbError> {
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

    pub fn get_channel_by_name(&self, channel: &str) -> Result<Option<Channel>, DbError> {
        self.conn
            .prepare("select * from channels where name = ?1 order by position")?
            .query_row([channel], |row| {
                let uuid = row.get(0)?;
                Ok(Channel {
                    uuid,
                    name: row.get(1)?,
                    position: row.get(2)?,
                    permissions: self.get_channel_permissions(uuid)?,
                })
            })
            .optional()
    }

    pub fn insert_channel(&self, channel: Channel) -> Result<(), DbError> {
        self.conn
            .prepare("insert into channels values (?1, ?2, ?3)")?
            .execute(params![channel.uuid, channel.name, channel.position])?;
        self.insert_channel_perms(&channel)
    }

    pub fn insert_user_groups(&self, user: &User) -> Result<(), DbError> {
        let mut insert_group = self
            .conn
            .prepare("insert into user_groups values (?1, ?2)")?;
        for g in &user.groups {
            insert_group.execute([user.uuid, *g])?;
        }
        Ok(())
    }

    pub fn insert_user(&self, user: User) -> Result<(), DbError> {
        self.conn
            .prepare("insert into users values (?1, ?2, ?3, ?4, ?5)")?
            .execute(params![
                user.uuid,
                user.name,
                user.pfp,
                user.group_uuid,
                user.password
            ])?;
        self.insert_user_groups(&user)
    }

    pub fn insert_sync_data(&self, data: &SyncData) -> Result<usize, DbError> {
        self.conn
            .prepare("insert into sync_data values (?1, ?2, ?3)")?
            .execute(params![data.user_uuid, data.uname, data.pfp])
    }

    pub fn insert_sync_server(&self, data: SyncServer) -> Result<usize, DbError> {
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

    pub fn update_user(&self, user: &User) -> Result<(), DbError> {
        self.conn
            .prepare("update users set name = ?1, pfp = ?2, group_uuid = ?3, password = ?4 where uuid = ?5")?
            .execute(params![user.name, user.pfp, user.group_uuid, user.password, user.uuid])?;

        // clear and re-add groups
        // TODO maybe not the most efficient
        self.conn
            .execute("delete from user_groups where user_uuid = ?1", [user.uuid])?;
        self.insert_user_groups(&user)
    }

    pub fn update_sync_data(&self, data: SyncData) -> Result<usize, DbError> {
        self.conn
            .prepare("update sync_data set uname = ?1, pfp = ?2 where user_uuid = ?3")?
            .execute(params![data.uname, data.pfp, data.user_uuid])
    }

    pub fn get_emoji(&self, uuid: Uuid) -> Result<Option<Emoji>, DbError> {
        self.conn
            .prepare("select * from emojis where uuid = ?1")?
            .query_row([uuid], |row| {
                Ok(Emoji {
                    uuid: row.get(0)?,
                    name: row.get(1)?,
                    data: row.get(2)?,
                })
            })
            .optional()
    }

    pub fn list_emoji(&self) -> Result<Vec<(String, Uuid)>, DbError> {
        self.conn
            .prepare("select name, uuid from emojis")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect()
    }

    pub fn edit_message(&self, uuid: Uuid, new_content: &str) -> Result<usize, DbError> {
        self.conn
            .prepare("update messages set content = ?1, edited = true where uuid = ?2")?
            .execute(params![new_content, uuid])
    }

    pub fn delete_message(&self, uuid: Uuid) -> Result<usize, DbError> {
        self.conn
            .prepare("delete from messages where uuid = ?1")?
            .execute([uuid])
    }

    pub fn delete_channel(&self, uuid: Uuid) -> Result<usize, DbError> {
        self.conn
            .prepare("delete from channels where uuid = ?1")?
            .execute([uuid])
    }

    pub fn clear_sync_servers_of(&self, user: Uuid) -> Result<usize, DbError> {
        self.conn
            .prepare("delete from sync_servers where user_uuid = ?1")?
            .execute([user])
    }

    pub fn get_sync_servers(&self, user: Uuid) -> Result<Vec<SyncServer>, DbError> {
        self.conn
            .prepare("select * from sync_servers where user_uuid = ?1 order by idx")?
            .query_map([user], |row| {
                Ok(SyncServer {
                    user_uuid: row.get(0)?,
                    uuid: row.get(1)?,
                    uname: row.get(2)?,
                    ip: row.get(3)?,
                    port: row.get(4)?,
                    pfp: row.get(5)?,
                    name: row.get(6)?,
                    idx: row.get(7)?,
                })
            })?
            .collect()
    }

    pub fn get_history(
        &self,
        channel: Uuid,
        num: u32,
        before_message: Option<Uuid>,
    ) -> Result<Vec<Message>, DbError> {
        let init_rowid = if let Some(uuid) = before_message {
            self.conn
                .prepare("select rowid from messages where uuid = ?1")?
                .query_row([uuid], |row| row.get(0))?
        } else {
            i32::MAX
        };
        self.conn.prepare("select * from messages where channel_uuid = ?1 and rowid < ?2 order by rowid limit ?3")?
            .query_map(params![channel, init_rowid, num], |row|
                Ok(Message {
                    uuid: row.get(0)?,
                    content: row.get(1)?,
                    author_uuid: row.get(2)?,
                    channel_uuid: row.get(3)?,
                    date: row.get(4)?,
                    edited: row.get(5)?,
                    reply: row.get(6)?,
                })
            )?.collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::permissions::Perm;

    use super::*;

    // #[test]
    // fn user_groups() {
    //     let s = init();
    //     let (mut u1, mut u2) = test_users();
    // }

    // TODO !!!!!! test new user/channel/group/groups_of/perms stuff
    // TODO !!!!! improve / increase migration testing
    // TODO !!! test that non-channel perms being given in a specific channel are ignored
    #[test]
    fn migration_simple() {
        let init = r#"
            BEGIN;
            CREATE TABLE version (
                version integer NOT NULL
            );
            INSERT INTO version VALUES(1);
            COMMIT;
        "#;
        let migrations = &[Migration {
            from: 1,
            to: 2,
            sql: r#"
                CREATE TABLE users (
                    uuid BigInt NOT NULL,
                    name text NOT NULL
                )
                "#,
            f: None,
        }];
        let sqlitedb = Connection::open_in_memory().expect("Unable to create a DB?");
        let shared = Shared::new(sqlitedb);
        shared.init_tables(init);
        shared.apply_migrations(migrations, 1, 2);
        assert_eq!(shared.get_db_version(), Some(2));
        assert!(shared
            .conn
            .execute_batch("INSERT INTO users VALUES (1, \"hello world\")")
            .is_ok());
        let q: Result<(i64, String), _> = shared.conn.query_row("select * from users", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        });

        assert!(q.is_ok());
        let r = q.unwrap();
        assert_eq!(r.0, 1);
        assert_eq!(r.1, "hello world");
    }

    #[test]
    fn migration_with_hook() {
        let init = r#"
            BEGIN;
            CREATE TABLE version (
                version integer NOT NULL
            );
            INSERT INTO version VALUES(1);
            COMMIT;
        "#;
        let migrations = &[Migration {
            from: 1,
            to: 2,
            sql: r#"
                CREATE TABLE users (
                    uuid BigInt NOT NULL,
                    name text NOT NULL
                )
                "#,
            f: Some(|conn: &Connection| {
                conn.execute_batch("insert into users values (2, \"hi\")")?;
                Ok(())
            }),
        }];
        let sqlitedb = Connection::open_in_memory().expect("Unable to create a DB?");
        let shared = Shared::new(sqlitedb);
        shared.init_tables(init);
        shared.apply_migrations(migrations, 1, 2);
        assert_eq!(shared.get_db_version(), Some(2));

        let q: Result<(i64, String), _> = shared.conn.query_row("select * from users", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        });

        assert!(q.is_ok());
        let r = q.unwrap();
        assert_eq!(r.0, 2);
        assert_eq!(r.1, "hi");
    }
    #[test]
    fn multiple_migrations() {
        let init = r#"
            BEGIN;
            CREATE TABLE version (
                version integer NOT NULL
            );
            INSERT INTO version VALUES(1);
            COMMIT;
        "#;
        let migrations = &[
            Migration {
                from: 1,
                to: 2,
                sql: r#"
                CREATE TABLE users (
                    uuid BigInt NOT NULL,
                    name text NOT NULL
                )
                "#,
                f: None,
            },
            Migration {
                from: 2,
                to: 3,
                sql: r#"
                CREATE TABLE messages (
                    uuid BigInt NOT NULL,
                    content text NOT NULL
                )
                "#,
                f: Some(|conn: &Connection| {
                    conn.execute_batch("insert into messages values (2, \"hi\")")?;
                    Ok(())
                }),
            },
        ];
        let sqlitedb = Connection::open_in_memory().expect("Unable to create a DB?");
        let shared = Shared::new(sqlitedb);
        shared.init_tables(init);
        shared.apply_migrations(migrations, 1, 3);

        assert_eq!(shared.get_db_version(), Some(3));
        assert!(shared
            .conn
            .execute_batch("INSERT INTO users VALUES (1, \"hello world\")")
            .is_ok());

        let q: Result<(i64, String), _> = shared.conn.query_row("select * from users", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        });

        assert!(q.is_ok());
        let r = q.unwrap();
        assert_eq!(r.0, 1);
        assert_eq!(r.1, "hello world");

        let q: Result<(i64, String), _> =
            shared.conn.query_row("select * from messages", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            });

        assert!(q.is_ok());
        let r = q.unwrap();
        assert_eq!(r.0, 2);
        assert_eq!(r.1, "hi");
    }

    fn init() -> Shared {
        let sqlitedb = Connection::open_in_memory().expect("Unable to create a DB?");
        let shared = Shared::new(sqlitedb);
        shared.init_db();
        shared
    }

    fn test_users() -> (User, User) {
        let mut user = User {
            uuid: gen_uuid(),
            name: "Test user".into(),
            pfp: "test pfp".into(),
            group_uuid: 0,
            password: "password".into(),
            groups: Vec::new(),
        };
        let user_2 = User {
            uuid: gen_uuid(),
            name: "User 2".into(),
            pfp: "test_pfp".into(),
            group_uuid: 0,
            password: "12345".into(),
            groups: Vec::new(),
        };
        user.groups.push(gen_uuid());
        user.groups.push(gen_uuid());
        user.groups.push(gen_uuid());
        (user, user_2)
    }

    fn test_channels() -> (Channel, Channel) {
        let mut c1 = Channel {
            name: "general-16.35".into(),
            uuid: gen_uuid(),
            position: 0,
            permissions: HashMap::new(),
        };
        let c2 = Channel {
            name: "memes".into(),
            uuid: gen_uuid(),
            position: 1,
            permissions: HashMap::new(),
        };

        c1.permissions.insert(
            PermableEntity::User(gen_uuid()),
            Permissions {
                modify_channels: Perm::Allow,
                modify_icon_name: Perm::Allow,
                modify_groups: Perm::Allow,
                modify_user_groups: Perm::Allow,
                ban_users: Perm::Allow,
                send_messages: Perm::Allow,
                read_messages: Perm::Allow,
                manage_messages: Perm::Allow,
                join_voice: Perm::Allow,
            },
        );

        (c1, c2)
    }

    fn test_messages(
        c1: &Channel,
        c2: &Channel,
        u1: &User,
        u2: &User,
    ) -> (Message, Message, Message) {
        let m1_uuid = gen_uuid();
        (
            Message {
                uuid: m1_uuid,
                content: "Hello world".into(),
                author_uuid: u1.uuid,
                channel_uuid: c1.uuid,
                date: 1359083513,
                edited: false,
                reply: None,
            },
            Message {
                uuid: gen_uuid(),
                content: "Goodbye world".into(),
                author_uuid: u1.uuid,
                channel_uuid: c2.uuid,
                date: 1359083514,
                edited: false,
                reply: None,
            },
            Message {
                uuid: gen_uuid(),
                content: "aster is the greatest lmfao".into(),
                author_uuid: u2.uuid,
                channel_uuid: c1.uuid,
                date: 1359083515,
                edited: false,
                reply: Some(m1_uuid),
            },
        )
    }

    fn init_with_users() -> (Shared, User, User) {
        let s = init();
        let (u1, u2) = test_users();
        s.insert_user(u1.clone()).unwrap();
        s.insert_user(u2.clone()).unwrap();
        (s, u1, u2)
    }

    #[test]
    fn insert_user() {
        let s = init();
        let (u1, u2) = test_users();
        assert!(s.insert_user(u1.clone()).is_ok());
        assert!(s.insert_user(u2.clone()).is_ok());
    }

    #[test]
    fn get_user_by_name() {
        let (s, u1, _) = init_with_users();
        let user_1_query = s.get_user_by_name(&u1.name);
        assert!(user_1_query.is_ok());
        let user_1_query = user_1_query.unwrap();
        assert!(user_1_query.is_some());
        let user_1_query = user_1_query.unwrap();
        assert_eq!(user_1_query.uuid, u1.uuid);
        assert_eq!(user_1_query.name, u1.name);
    }

    #[test]
    fn get_nonexistant_user_by_name() {
        let (s, _, _) = init_with_users();
        let user_1_query = s.get_user_by_name("I am a nonexistant user");
        assert!(user_1_query.is_ok());
        let user_1_query = user_1_query.unwrap();
        assert!(user_1_query.is_none());
    }

    #[test]
    fn get_user_by_uuid() {
        let (s, u1, _) = init_with_users();

        let user_1_query = s.get_user(u1.uuid);
        assert!(user_1_query.is_ok());
        let user_1_query = user_1_query.unwrap();
        assert!(user_1_query.is_some());
        let user_1_query = user_1_query.unwrap();
        assert_eq!(user_1_query.uuid, u1.uuid);
        assert_eq!(user_1_query.name, u1.name);
    }

    #[test]
    fn get_nonexistant_user_by_uuid() {
        let (s, _, _) = init_with_users();

        let user_1_query = s.get_user(gen_uuid());
        assert!(user_1_query.is_ok());
        let user_1_query = user_1_query.unwrap();
        assert!(user_1_query.is_none());
    }

    #[test]
    fn list_users() {
        let (s, u1, u2) = init_with_users();

        let users = s.get_users();
        assert!(users.is_ok());
        let users = users.unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].uuid, u1.uuid);
        assert_eq!(users[1].uuid, u2.uuid);
    }

    #[test]
    fn update_user() {
        let (s, u1, _) = init_with_users();
        assert_eq!(u1.name, "Test user");
        let new_u1 = User {
            uuid: u1.uuid,
            name: "Test user updated".into(),
            pfp: "pfp2".into(),
            group_uuid: 2,
            password: "abcde".into(),
            groups: Vec::new(),
        };
        assert!(s.update_user(&new_u1).is_ok());
        let user_1_query = s.get_user(u1.uuid);
        assert!(user_1_query.is_ok());
        let user_1_query = user_1_query.unwrap();
        assert!(user_1_query.is_some());
        assert_eq!(user_1_query.unwrap(), new_u1);
    }

    #[test]
    fn initial_channel_is_general() {
        let s = init();
        let chans = s.get_channels();
        assert!(chans.is_ok());
        let chans = chans.unwrap();
        assert_eq!(chans.len(), 1);
        assert_eq!(chans[0].name, "general");
    }

    #[test]
    fn insert_channels() {
        let s = init();
        let (c1, c2) = test_channels();
        assert!(s.insert_channel(c1.clone()).is_ok());
        assert!(s.insert_channel(c2).is_ok());
    }

    #[test]
    fn get_channel_by_uuid() {
        let s = init();
        let (c1, c2) = test_channels();
        assert!(s.insert_channel(c1.clone()).is_ok());
        assert!(s.insert_channel(c2).is_ok());

        let c1_q = s.get_channel(&c1.uuid);
        assert!(c1_q.is_ok());
        let c1_q = c1_q.unwrap();
        assert!(c1_q.is_some());
        let c1_q = c1_q.unwrap();
        assert_eq!(c1_q.uuid, c1.uuid);
        assert_eq!(c1_q.name, c1.name);
    }

    #[test]
    fn get_channel_by_name() {
        let s = init();
        let (c1, c2) = test_channels();
        assert!(s.insert_channel(c1.clone()).is_ok());
        assert!(s.insert_channel(c2).is_ok());

        let c1_q = s.get_channel_by_name(&c1.name);
        assert!(c1_q.is_ok());
        let c1_q = c1_q.unwrap();
        assert!(c1_q.is_some());
        let c1_q = c1_q.unwrap();
        assert_eq!(c1_q.uuid, c1.uuid);
        assert_eq!(c1_q.name, c1.name);
    }

    #[test]
    fn get_nonexistant_channel_by_uuid() {
        let s = init();
        let (c1, c2) = test_channels();
        assert!(s.insert_channel(c1.clone()).is_ok());
        assert!(s.insert_channel(c2).is_ok());

        let c1_q = s.get_channel(&gen_uuid());
        assert!(c1_q.is_ok());
        let c1_q = c1_q.unwrap();
        assert!(c1_q.is_none());
    }

    #[test]
    fn get_nonexistant_channel_by_name() {
        let s = init();
        let (c1, c2) = test_channels();
        assert!(s.insert_channel(c1.clone()).is_ok());
        assert!(s.insert_channel(c2).is_ok());

        let c1_q = s.get_channel_by_name("this channel does not exist");
        assert!(c1_q.is_ok());
        let c1_q = c1_q.unwrap();
        assert!(c1_q.is_none());
    }

    #[test]
    fn channel_exists() {
        let s = init();
        let c1 = Channel {
            name: "general-16.35".into(),
            uuid: gen_uuid(),
            position: 0,
            permissions: HashMap::new(),
        };

        assert!(s.insert_channel(c1.clone()).is_ok());
        assert!(s.channel_exists(&c1.uuid).is_ok());
        assert_eq!(s.channel_exists(&c1.uuid).unwrap(), true);
    }

    #[test]
    fn channel_doesnt_exist() {
        let s = init();
        let c1 = Channel {
            name: "general-16.35".into(),
            uuid: gen_uuid(),
            position: 0,
            permissions: HashMap::new(),
        };

        let not_existing_uuid = gen_uuid();
        assert!(s.insert_channel(c1.clone()).is_ok());
        assert!(s.channel_exists(&not_existing_uuid).is_ok());
        assert_eq!(s.channel_exists(&not_existing_uuid).unwrap(), false);
    }

    #[test]
    fn duplicate_channel() {
        let s = init();
        let c1 = Channel {
            name: "general-16.35".into(),
            uuid: gen_uuid(),
            position: 0,
            permissions: HashMap::new(),
        };
        assert!(s.insert_channel(c1.clone()).is_ok());
        assert!(s.insert_channel(c1.clone()).is_err());
    }

    #[test]
    fn delete_channel() {
        let s = init();
        let c1 = Channel {
            name: "general-16.35".into(),
            uuid: gen_uuid(),
            position: 0,
            permissions: HashMap::new(),
        };
        assert!(s.insert_channel(c1.clone()).is_ok());
        assert!(s.delete_channel(c1.uuid).is_ok());
        assert!(s.get_channel(&c1.uuid).is_ok_and(|c| c.is_none()));
    }
    #[test]
    fn update_channel() {
        let s = init();
        let c1 = Channel {
            name: "general-16.35".into(),
            uuid: gen_uuid(),
            position: 1,
            permissions: HashMap::new(),
        };
        assert!(s.insert_channel(c1.clone()).is_ok());
        let c2 = Channel {
            name: "random-shit".into(),
            uuid: c1.uuid,
            position: 1,
            permissions: HashMap::new(),
        };
        assert!(s.update_channel(&c2).is_ok());
        assert!(s.get_channel(&c1.uuid).is_ok());
        assert!(s.get_channel(&c1.uuid).unwrap().is_some());
        assert_eq!(s.get_channel(&c1.uuid).unwrap().unwrap(), c2);
    }

    fn init_with_msgs(
        insert: bool,
    ) -> (
        Shared,
        Message,
        Message,
        Message,
        Channel,
        Channel,
        User,
        User,
    ) {
        let s = init();
        let (c1, c2) = test_channels();
        let (u1, u2) = test_users();
        let (m1, m2, m3) = test_messages(&c1, &c2, &u1, &u2);
        s.insert_channel(c1.clone()).unwrap();
        s.insert_channel(c2.clone()).unwrap();
        s.insert_user(u1.clone()).unwrap();
        s.insert_user(u2.clone()).unwrap();
        if insert {
            s.add_to_history(&m1).unwrap();
            s.add_to_history(&m2).unwrap();
            s.add_to_history(&m3).unwrap();
        }
        (s, m1, m2, m3, c1, c2, u1, u2)
    }

    #[test]
    fn insert_message() {
        let (s, m1, m2, m3, _, _, _, _) = init_with_msgs(false);
        assert!(s.add_to_history(&m1).is_ok());
        assert!(s.add_to_history(&m2).is_ok());
        assert!(s.add_to_history(&m3).is_ok());
    }

    #[test]
    fn message_exists() {
        let (s, _, m2, _, _, _, _, _) = init_with_msgs(true);
        assert!(s.message_exists(&m2.uuid).is_ok());
        assert_eq!(s.message_exists(&m2.uuid).unwrap(), true);
    }

    #[test]
    fn message_doesnt_exist() {
        let (s, _, _, _, _, _, _, _) = init_with_msgs(true);
        let not_existing_uuid = gen_uuid();
        assert!(s.message_exists(&not_existing_uuid).is_ok());
        assert_eq!(s.message_exists(&not_existing_uuid).unwrap(), false);
    }

    #[test]
    fn get_message() {
        let (s, _m1, m2, _m3, _c1, _c2, _u1, _u2) = init_with_msgs(true);
        let mq = s.get_message(m2.uuid);
        assert!(mq.is_ok());
        let mq = mq.unwrap();
        assert!(mq.is_some());
        let mq = mq.unwrap();
        assert_eq!(mq, m2);
    }

    #[test]
    fn get_nonexistant_message() {
        let (s, _m1, _m2, _m3, _c1, _c2, _u1, _u2) = init_with_msgs(true);
        let mq = s.get_message(gen_uuid());
        assert!(mq.is_ok());
        let mq = mq.unwrap();
        assert!(mq.is_none());
    }

    #[test]
    fn edit_message() {
        let (s, _, m2, _, _, _, _, _) = init_with_msgs(true);
        assert_eq!(m2.content, "Goodbye world");
        assert!(s.edit_message(m2.uuid, "Hello world").is_ok());
        let mq = s.get_message(m2.uuid).unwrap().unwrap();
        assert_eq!(mq.content, "Hello world");
        assert_eq!(mq.edited, true);
    }

    #[test]
    fn delete_message() {
        let (s, _, m2, _, _, _, _, _) = init_with_msgs(true);
        assert!(s.get_message(m2.uuid).is_ok_and(|o| o.is_some()));
        assert!(s.delete_message(m2.uuid).is_ok());
        assert!(s.get_message(m2.uuid).is_ok_and(|o| o.is_none()));
    }

    #[test]
    fn get_history() {
        let (s, m1, _, _, c1, _, _, _) = init_with_msgs(true);
        let h = s.get_history(c1.uuid, 5, None);
        assert!(h.is_ok());
        let h = h.unwrap();
        assert!(h.len() == 2);
        assert_eq!(h[0], m1);
    }

    #[test]
    fn get_history_limited() {
        let (s, _, _, _, c1, _, _, _) = init_with_msgs(true);
        let h = s.get_history(c1.uuid, 1, None);
        assert!(h.is_ok());
        let h = h.unwrap();
        assert!(h.len() == 1);
    }

    #[test]
    fn get_history_before() {
        let (s, m1, _, m3, c1, _, _, _) = init_with_msgs(true);
        let h = s.get_history(c1.uuid, 5, Some(m3.uuid));
        assert!(h.is_ok());
        let h = h.unwrap();
        assert!(h.len() == 1);
        assert_eq!(h[0], m1);
    }

    #[test]
    fn insert_sync_data() {
        let s = init();
        let d = SyncData {
            user_uuid: gen_uuid(),
            uname: "Hi".into(),
            pfp: "test_pfp".into(),
        };
        assert!(s.insert_sync_data(&d).is_ok());
    }

    #[test]
    fn get_sync_data() {
        let s = init();
        let d = SyncData {
            user_uuid: gen_uuid(),
            uname: "Hi".into(),
            pfp: "test_pfp".into(),
        };
        assert!(s.insert_sync_data(&d).is_ok());
        assert!(s.get_sync_data(&d.user_uuid).is_ok());
        assert!(s.get_sync_data(&d.user_uuid).unwrap().is_some());
        assert_eq!(s.get_sync_data(&d.user_uuid).unwrap().unwrap(), d);
    }

    #[test]
    fn update_sync_data() {
        let s = init();
        let d = SyncData {
            user_uuid: gen_uuid(),
            uname: "Hi".into(),
            pfp: "test_pfp".into(),
        };
        let d2 = SyncData {
            user_uuid: d.user_uuid,
            uname: "Hello".into(),
            pfp: "test_pfp_2".into(),
        };
        assert!(s.insert_sync_data(&d).is_ok());
        assert!(s.update_sync_data(d2.clone()).is_ok());
        assert!(s.get_sync_data(&d.user_uuid).is_ok());
        assert!(s.get_sync_data(&d.user_uuid).unwrap().is_some());
        assert_eq!(s.get_sync_data(&d.user_uuid).unwrap().unwrap(), d2);
    }

    #[test]
    fn insert_sync_server() {
        let s = init();
        let ss = SyncServer {
            user_uuid: gen_uuid(),
            uuid: Some(gen_uuid()),
            uname: "Hi".into(),
            ip: "192.168.0.1".into(),
            port: 6942,
            pfp: None,
            name: Some("test server".into()),
            idx: 1,
        };
        assert!(s.insert_sync_server(ss).is_ok());
    }
    #[test]
    fn get_sync_server() {
        let s = init();
        let ss = SyncServer {
            user_uuid: gen_uuid(),
            uuid: Some(gen_uuid()),
            uname: "Hi".into(),
            ip: "192.168.0.1".into(),
            port: 6942,
            pfp: None,
            name: Some("test server".into()),
            idx: 1,
        };
        let ss2 = SyncServer {
            user_uuid: gen_uuid(),
            uuid: Some(gen_uuid()),
            uname: "iH".into(),
            ip: "192.168.0.128".into(),
            port: 2345,
            pfp: None,
            name: Some("another server".into()),
            idx: 1,
        };
        assert!(s.insert_sync_server(ss.clone()).is_ok());
        assert!(s.insert_sync_server(ss2).is_ok());

        let ss1 = s.get_sync_servers(ss.user_uuid);
        assert!(ss1.is_ok());
        let ss1 = ss1.unwrap();
        assert!(ss1.len() == 1);
        assert_eq!(ss1[0], ss);
    }
    #[test]
    fn clear_sync_server() {
        let s = init();
        let ss = SyncServer {
            user_uuid: gen_uuid(),
            uuid: Some(gen_uuid()),
            uname: "Hi".into(),
            ip: "192.168.0.1".into(),
            port: 6942,
            pfp: None,
            name: Some("test server".into()),
            idx: 1,
        };
        let mut ss2 = ss.clone();
        ss2.idx = 2;
        assert!(s.insert_sync_server(ss.clone()).is_ok());
        assert!(s.insert_sync_server(ss2).is_ok());
        assert!(s.clear_sync_servers_of(ss.user_uuid).is_ok());

        let ss1 = s.get_sync_servers(ss.user_uuid);
        assert!(ss1.is_ok());
        let ss1 = ss1.unwrap();
        assert!(ss1.len() == 0);
    }
}
