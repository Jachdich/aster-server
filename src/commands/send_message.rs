use rusqlite::params;
use std::sync::Arc;

use crate::{attachments, shared::Shared};

#[derive(serde::Deserialize)]
pub struct SendPayload {
    pub channel_uuid: i64,
    pub content: String,
    pub attachment_ids: Vec<i64>,
}

pub async fn handle(
    shared: Arc<Shared>,
    author_uuid: i64,
    payload: SendPayload,
) -> Result<(), crate::error::Error> {
    let conn = &shared.conn;

    conn.execute(
        "INSERT INTO messages (content, author_uuid, channel_uuid, date, edited)
         VALUES (?1, ?2, ?3, strftime('%s','now'), 0)",
        params![payload.content, author_uuid, payload.channel_uuid],
    )?;
    let message_id = conn.last_insert_rowid();

    // link any uploaded files
    for (idx, fid) in payload.attachment_ids.iter().enumerate() {
        attachments::attach_file(conn, message_id, *fid, idx as i32)?;
    }

    // TODO: broadcast websocket event here

    Ok(())
}
