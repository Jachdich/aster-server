//smol helper that i think can be used for the message creating path

use rusqlite::{params, Connection};

// Inserts a row into `attachments' keeping the order the client sent.
// Calling this once per `(message_id, file_id)` pair after the message row is commited.
pub fn attach_file(
    conn: &Connection,
    message_id: i64,
    file_id: i64,
    order_index: i32,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO attachments (message_id, file_id, order_index) VALUES (?1, ?2, ?3)",
        params![message_id, file_id, order_index],
    )?;
    Ok(())
}
