// new file woohoo
use anyhow::{Context, Result};
use rusqlite::Error as SqlErr;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub struct FileMeta {
    pub id: i64,
    pub uploader_id: i64,
    pub kind: String,
    pub content_type: String,
    pub byte_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub sha256: Vec<u8>,
    pub created_at: i64,
}

//builder object, only used at insert-time
// holds both the path to the just-written disk file and metadata extracted
pub struct NewFile<'a> {
    pub conn: &'a Connection,
    pub uploader_id: i64,
    pub kind: &'a str,         // "image" / "video" / …
    pub content_type: &'a str, // MIME string
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub disk_path: &'a Path, // already written
}

//streams the file once to compute sha256 and byte size, then inserts a new row into 'files' and returns auto gen id.
impl<'a> NewFile<'a> {
    pub fn insert(self) -> Result<i64> {
        // --- hash & size in one pass ---
        let mut file =
            File::open(self.disk_path).with_context(|| format!("opening {:?}", self.disk_path))?;

        let mut hasher = Sha256::new();
        let mut buf = [0u8; 8192];
        let mut total = 0u64;

        loop {
            let n = file
                .read(&mut buf)
                .with_context(|| format!("reading {:?}", self.disk_path))?;
            if n == 0 {
                break;
            }
            total += n as u64;
            hasher.update(&buf[..n]);
        }

        let sha256 = hasher.finalize().to_vec();
        let ts = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO files \
             (uploader_id, kind, content_type, byte_size, width, height, sha256, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                self.uploader_id,
                self.kind,
                self.content_type,
                total as i64,
                self.width,
                self.height,
                sha256,
                ts,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }
}

//fetch a single filemeta by id. returns Ok(None) if the row doesnt exist
impl FileMeta {
    pub fn by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "SELECT id,uploader_id,kind,content_type,byte_size,width,height,sha256,created_at FROM files WHERE id=?",
            [id],
            |r| {
                Ok(FileMeta {
                    id: r.get(0)?,
                    uploader_id: r.get(1)?,
                    kind: r.get(2)?,
                    content_type: r.get(3)?,
                    byte_size: r.get(4)?,
                    width: r.get(5)?,
                    height: r.get(6)?,
                    sha256: r.get(7)?,
                    created_at: r.get(8)?,
                })
            },
        )
        .optional()
    }
}
