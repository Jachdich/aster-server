use crate::schema::messages;
use crate::schema::channels;
use crate::schema::users;
use crate::schema::groups;

#[derive(Queryable, Insertable, Clone)]
#[table_name="channels"]
pub struct Channel {
    pub uuid: i64,
    pub name: String,
}

#[derive(Queryable, Insertable, Clone)]
#[table_name="messages"]
pub struct CookedMessage {
    pub uuid: i64,
    pub content: String,
    pub author_uuid: i64,
    pub channel_uuid: i64,
    pub date: i32,
}

#[derive(Queryable, Insertable, Clone)]
#[table_name="users"]
pub struct User {
    pub uuid: i64,
    pub name: String,
    pub pfp: String,
    pub group_uuid: i64,
}

#[derive(Queryable, Insertable, Clone)]
#[table_name="groups"]
pub struct Group {
    pub uuid: i64,
    pub permissions: i64,
    pub name: String,
    pub colour: i32,
}

impl CookedMessage {
    pub fn as_json(&self) -> json::JsonValue {
        return json::object!{content: self.content.clone(), user: self.user};
    }
    pub fn from_json(value: &json::JsonValue) -> Self {
        CookedMessage{
            content: value["content"].to_string(),
            user: value["user"].as_u64().unwrap(),
        }
    }
}

impl User {
    pub fn as_json(&self) -> json::JsonValue {
        return json::object!{name: self.name.clone(), uuid: self.uuid, pfp: self.pfp.clone()};
    }
    pub fn from_json(value: &json::JsonValue) -> Self {
        User {
            name: value["name"].as_str().unwrap().to_string(),
            pfp: value["pfp"].as_str().unwrap().to_string(),
            uuid: value["uuid"].as_u64().unwrap(),
        }
    }
}