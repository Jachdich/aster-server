use super::schema::messages;

#[derive(Insertable)]
#[table_name="messages"]
pub struct Message<'a> {
    pub uuid: &'a i64,
    pub content: &'a str,
    pub author_uuid: &'a i64,
    pub channel_uuid: &'a i64,
    pub date: &'a i64,
}
