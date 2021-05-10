use crate::schema::messages;

#[derive(Clone)]
pub struct RawMessage {
    pub content: String,
}

#[derive(Queryable, Clone)]
pub struct CookedMessage {
    pub uuid: i64,
    pub content: String,
    pub author_uuid: i64,
    pub channel_uuid: i64,
    pub date: i32,
    pub rowid: i64,
}

#[derive(Insertable, Clone)]
#[table_name="messages"]
pub struct CookedMessageInsertable {
    pub uuid: i64,
    pub content: String,
    pub author_uuid: i64,
    pub channel_uuid: i64,
    pub date: i32,
}

#[derive(Clone)]
pub enum MessageType {
    Raw(RawMessage),
    Cooked(CookedMessage),
}

pub enum Message {
    Broadcast(MessageType),
    Received(MessageType),
}

impl CookedMessage {
    pub fn as_json(&self) -> json::JsonValue {
        return json::object!{
            uuid: self.uuid,
            content: self.content.clone(),
            author_uuid: self.author_uuid,
            channel_uuid: self.channel_uuid,
            date: self.date};
    }
    pub fn from_json(value: &json::JsonValue) -> Self {
        CookedMessage{
            uuid: value["uuid"].as_i64().unwrap(),
            content: value["content"].to_string(),
            author_uuid: value["author_uuid"].as_i64().unwrap(),
            channel_uuid: value["channel_uuid"].as_i64().unwrap(),
            date: value["date"].as_i32().unwrap(),
            rowid: 0,
        }
    }
}

impl CookedMessageInsertable {
    pub fn new(msg: CookedMessage) -> Self {
        return Self {
            uuid: msg.uuid,
            content: msg.content,
            author_uuid: msg.author_uuid,
            channel_uuid: msg.channel_uuid,
            date: msg.date,
        };
    }
}
