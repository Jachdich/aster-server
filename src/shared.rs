use crate::serverproperties::ServerProperties;
use crate::sharedchannel::SharedChannel;
use crate::schema;
use std::collections::HashMap;
use diesel::prelude::*;
use crate::models::*;
use crate::message::*;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use crate::peer::Pontoon;

pub struct Shared {
    pub channels: HashMap<i64, SharedChannel>,
    pub online: Vec<i64>,
    pub conn: SqliteConnection,
    pub properties: ServerProperties,
    pub peers: Vec<Pontoon>,
}

impl Shared {
    pub fn new() -> Self {
        let channels: HashMap<i64, SharedChannel> = HashMap::new();
        let sqlitedb = SqliteConnection::establish("aster.db").expect(&format!("Error connecting to the database file {}", "aster.db"));
        Shared {
            channels,
            online: Vec::new(),
            conn: sqlitedb,
            properties: ServerProperties::load(),
            peers: Vec::new(),
        }
    }

    pub fn load(&mut self) {
        let channels = self.get_channels();
        if channels.len() == 0 {
            let new_channel = Channel::new("general");
            self.channels.insert(new_channel.uuid, SharedChannel::new(new_channel.clone()));
            self.insert_channel(new_channel);
        } else {
            for channel in channels {
                self.channels.insert(channel.uuid, SharedChannel::new(channel));
            }
        }
    }

    pub fn broadcast_unread(&self, target_channel: i64, why_the_fuck_do_i_need_this: &tokio::sync::MutexGuard<'_, Shared>) {
    	let name = self.channels.get(&target_channel).unwrap().channel.name.to_owned();
    	for (uuid, channel) in &self.channels {
    		if uuid != &target_channel {
    			channel.broadcast(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255)), 0),
    			MessageType::Raw(RawMessage{
    				content: json::object!{command: "unread", channel: name.to_owned()}.dump()
    			}), why_the_fuck_do_i_need_this);
    		}
    	}
    }

    pub fn broadcast_to_all(&self, message: MessageType, why_the_fuck_do_i_need_this: &tokio::sync::MutexGuard<'_, Shared>) {
    	for (_uuid, channel) in &self.channels {
			channel.broadcast(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255)), 0),
			message.clone(), why_the_fuck_do_i_need_this);
     	}
    }

    // fn save(&mut self) {
        // 
    // }

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

    pub fn get_channel(&self, channel: &i64) -> Channel {
        let mut results = schema::channels::table
            .filter(schema::channels::uuid.eq(channel))
            .limit(1)
            .load::<Channel>(&self.conn)
            .expect("Channel does not exist");

        return results.remove(0);
    }

    pub fn get_channel_by_name(&self, channel: &String) -> Channel {
        let mut results = schema::channels::table
            .filter(schema::channels::name.eq(channel))
            .limit(1)
            .load::<Channel>(&self.conn)
            .expect("Channel does not exist");

        return results.remove(0);
    }

    pub fn insert_channel(&self, channel: Channel) {
        let _ = diesel::insert_into(schema::channels::table)
            .values(&channel)
            .execute(&self.conn)
            .expect("Error appending channel");
    }

    pub fn insert_user(&self, user: User) {
        let _ = diesel::insert_into(schema::users::table)
            .values(&user)
            .execute(&self.conn)
            .expect("Error appending user");
    }

    pub fn update_user(&self, user: User) {
        diesel::update(schema::users::table.find(user.uuid))
            .set((schema::users::name.eq(user.name),
            schema::users::pfp.eq(user.pfp),
            schema::users::group_uuid.eq(user.group_uuid)))
            .execute(&self.conn)
            .expect(&format!("Unable to find user {}", user.uuid));
    }
}
