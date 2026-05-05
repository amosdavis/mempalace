use crate::error::MpError;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawerMetadata {
    pub drawer_id: String,
    pub wing: String,
    pub room: String,
    pub source_file: Option<String>,
    pub source_mtime: Option<f64>,
    pub chunk_index: Option<i64>,
    pub type_: String,
    pub created_at: f64,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drawer {
    pub metadata: DrawerMetadata,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelRecord {
    pub tunnel_id: String,
    pub from_wing: String,
    pub from_room: String,
    pub to_wing: String,
    pub to_room: String,
    pub created_at: f64,
    pub note: Option<String>,
}

pub struct PalaceStore {
    conn: Connection,
}

impl PalaceStore {
    pub fn open(palace_path: &str) -> Result<Self, MpError> {
        std::fs::create_dir_all(palace_path)?;
        let db_path = std::path::Path::new(palace_path).join("palace.sqlite3");
        let conn = Connection::open(&db_path)?;
        // Wait up to 5 seconds for write-lock contention (background miners, git hooks)
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let store = Self { conn };
        store.create_schema()?;
        Ok(store)
    }

    fn create_schema(&self) -> Result<(), MpError> {
        self.conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS drawers (
                drawer_id   TEXT PRIMARY KEY,
                wing        TEXT NOT NULL,
                room        TEXT NOT NULL,
                source_file TEXT,
                source_mtime REAL,
                chunk_index INTEGER,
                type        TEXT NOT NULL DEFAULT 'text',
                created_at  REAL NOT NULL,
                content_hash TEXT NOT NULL,
                content     TEXT NOT NULL,
                embedding   BLOB
            );
            CREATE INDEX IF NOT EXISTS idx_drawers_wing ON drawers(wing);
            CREATE INDEX IF NOT EXISTS idx_drawers_room ON drawers(room);
            CREATE INDEX IF NOT EXISTS idx_drawers_wing_room ON drawers(wing, room);
            CREATE INDEX IF NOT EXISTS idx_drawers_source_file ON drawers(source_file);

            CREATE VIRTUAL TABLE IF NOT EXISTS drawers_fts USING fts5(
                drawer_id UNINDEXED,
                content,
                content='drawers',
                content_rowid='rowid'
            );

            CREATE TRIGGER IF NOT EXISTS drawers_ai AFTER INSERT ON drawers BEGIN
                INSERT INTO drawers_fts(rowid, drawer_id, content)
                VALUES (new.rowid, new.drawer_id, new.content);
            END;

            CREATE TRIGGER IF NOT EXISTS drawers_ad AFTER DELETE ON drawers BEGIN
                INSERT INTO drawers_fts(drawers_fts, rowid, drawer_id, content)
                VALUES ('delete', old.rowid, old.drawer_id, old.content);
            END;

            CREATE TRIGGER IF NOT EXISTS drawers_au AFTER UPDATE ON drawers BEGIN
                INSERT INTO drawers_fts(drawers_fts, rowid, drawer_id, content)
                VALUES ('delete', old.rowid, old.drawer_id, old.content);
                INSERT INTO drawers_fts(rowid, drawer_id, content)
                VALUES (new.rowid, new.drawer_id, new.content);
            END;

            CREATE TABLE IF NOT EXISTS tunnels (
                tunnel_id  TEXT PRIMARY KEY,
                from_wing  TEXT NOT NULL,
                from_room  TEXT NOT NULL,
                to_wing    TEXT NOT NULL,
                to_room    TEXT NOT NULL,
                created_at REAL NOT NULL,
                note       TEXT
            );
        "#)?;
        Ok(())
    }

    pub fn upsert_drawer(
        &self,
        wing: &str,
        room: &str,
        content: &str,
        type_: &str,
        source_file: Option<&str>,
        source_mtime: Option<f64>,
        chunk_index: Option<i64>,
        embedding: Option<&[f32]>,
    ) -> Result<String, MpError> {
        let content_hash = {
            let mut h = Sha256::new();
            h.update(content.as_bytes());
            hex::encode(h.finalize())
        };
        let drawer_id = {
            let mut h = Sha256::new();
            h.update(wing.as_bytes());
            h.update(room.as_bytes());
            h.update(content_hash.as_bytes());
            hex::encode(h.finalize())[..16].to_string()
        };
        let created_at = Utc::now().timestamp_millis() as f64 / 1000.0;
        let embedding_bytes: Option<Vec<u8>> = embedding.map(|e| {
            e.iter().flat_map(|f| f.to_le_bytes()).collect()
        });

        self.conn.execute(
            r#"INSERT INTO drawers
                   (drawer_id, wing, room, source_file, source_mtime, chunk_index,
                    type, created_at, content_hash, content, embedding)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
               ON CONFLICT(drawer_id) DO UPDATE SET
                   content       = excluded.content,
                   content_hash  = excluded.content_hash,
                   source_mtime  = excluded.source_mtime,
                   embedding     = excluded.embedding"#,
            params![
                drawer_id, wing, room, source_file, source_mtime,
                chunk_index, type_, created_at, content_hash, content, embedding_bytes
            ],
        )?;
        Ok(drawer_id)
    }

    pub fn get_drawer(&self, drawer_id: &str) -> Result<Drawer, MpError> {
        self.conn.query_row(
            r#"SELECT drawer_id, wing, room, source_file, source_mtime,
                      chunk_index, type, created_at, content_hash, content
               FROM drawers WHERE drawer_id = ?1"#,
            params![drawer_id],
            |row| {
                Ok(Drawer {
                    metadata: DrawerMetadata {
                        drawer_id:    row.get(0)?,
                        wing:         row.get(1)?,
                        room:         row.get(2)?,
                        source_file:  row.get(3)?,
                        source_mtime: row.get(4)?,
                        chunk_index:  row.get(5)?,
                        type_:        row.get(6)?,
                        created_at:   row.get(7)?,
                        content_hash: row.get(8)?,
                    },
                    content: row.get(9)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows =>
                MpError::NotFound(format!("Drawer {drawer_id} not found")),
            other => MpError::Storage(other),
        })
    }

    fn row_to_metadata(row: &rusqlite::Row) -> rusqlite::Result<DrawerMetadata> {
        Ok(DrawerMetadata {
            drawer_id:    row.get(0)?,
            wing:         row.get(1)?,
            room:         row.get(2)?,
            source_file:  row.get(3)?,
            source_mtime: row.get(4)?,
            chunk_index:  row.get(5)?,
            type_:        row.get(6)?,
            created_at:   row.get(7)?,
            content_hash: row.get(8)?,
        })
    }

    pub fn list_drawers(
        &self,
        wing: Option<&str>,
        room: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<DrawerMetadata>, MpError> {
        let limit = limit.unwrap_or(100);
        let base = "SELECT drawer_id, wing, room, source_file, source_mtime, \
                           chunk_index, type, created_at, content_hash \
                    FROM drawers";
        let rows = match (wing, room) {
            (Some(w), Some(r)) => {
                let sql = format!("{base} WHERE wing = ?1 AND room = ?2 ORDER BY created_at DESC LIMIT ?3");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![w, r, limit], Self::row_to_metadata)?
                         .collect::<Result<Vec<_>, _>>()?;
                v
            }
            (Some(w), None) => {
                let sql = format!("{base} WHERE wing = ?1 ORDER BY created_at DESC LIMIT ?2");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![w, limit], Self::row_to_metadata)?
                         .collect::<Result<Vec<_>, _>>()?;
                v
            }
            (None, Some(r)) => {
                let sql = format!("{base} WHERE room = ?1 ORDER BY created_at DESC LIMIT ?2");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![r, limit], Self::row_to_metadata)?
                         .collect::<Result<Vec<_>, _>>()?;
                v
            }
            (None, None) => {
                let sql = format!("{base} ORDER BY created_at DESC LIMIT ?1");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![limit], Self::row_to_metadata)?
                         .collect::<Result<Vec<_>, _>>()?;
                v
            }
        };
        Ok(rows)
    }

    /// Like `list_drawers` but fetches content in the same query, avoiding N+1 round-trips.
    pub fn list_drawers_with_content(
        &self,
        wing: Option<&str>,
        room: Option<&str>,
        limit: i64,
    ) -> Result<Vec<(DrawerMetadata, String)>, MpError> {
        let base = "SELECT drawer_id, wing, room, source_file, source_mtime, \
                           chunk_index, type, created_at, content_hash, content \
                    FROM drawers";
        let rows = match (wing, room) {
            (Some(w), Some(r)) => {
                let sql = format!("{base} WHERE wing = ?1 AND room = ?2 ORDER BY created_at DESC LIMIT ?3");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![w, r, limit], |row| {
                    Ok((Self::row_to_metadata(row)?, row.get(9)?))
                })?.collect::<Result<Vec<_>, _>>()?;
                v
            }
            (Some(w), None) => {
                let sql = format!("{base} WHERE wing = ?1 ORDER BY created_at DESC LIMIT ?2");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![w, limit], |row| {
                    Ok((Self::row_to_metadata(row)?, row.get(9)?))
                })?.collect::<Result<Vec<_>, _>>()?;
                v
            }
            (None, Some(r)) => {
                let sql = format!("{base} WHERE room = ?1 ORDER BY created_at DESC LIMIT ?2");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![r, limit], |row| {
                    Ok((Self::row_to_metadata(row)?, row.get(9)?))
                })?.collect::<Result<Vec<_>, _>>()?;
                v
            }
            (None, None) => {
                let sql = format!("{base} ORDER BY created_at DESC LIMIT ?1");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![limit], |row| {
                    Ok((Self::row_to_metadata(row)?, row.get(9)?))
                })?.collect::<Result<Vec<_>, _>>()?;
                v
            }
        };
        Ok(rows)
    }

    pub fn delete_drawer(&self, drawer_id: &str) -> Result<bool, MpError> {
        let n = self.conn.execute(
            "DELETE FROM drawers WHERE drawer_id = ?1",
            params![drawer_id],
        )?;
        Ok(n > 0)
    }

    pub fn count_drawers(&self, wing: Option<&str>) -> Result<i64, MpError> {
        match wing {
            Some(w) => Ok(self.conn.query_row(
                "SELECT COUNT(*) FROM drawers WHERE wing = ?1",
                params![w],
                |r| r.get(0),
            )?),
            None => Ok(self.conn.query_row(
                "SELECT COUNT(*) FROM drawers",
                [],
                |r| r.get(0),
            )?),
        }
    }

    pub fn list_wings(&self) -> Result<Vec<String>, MpError> {
        let mut s = self.conn.prepare("SELECT DISTINCT wing FROM drawers ORDER BY wing")?;
        let v = s.query_map([], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }

    pub fn list_rooms(&self, wing: &str) -> Result<Vec<String>, MpError> {
        let mut s = self.conn.prepare(
            "SELECT DISTINCT room FROM drawers WHERE wing = ?1 ORDER BY room"
        )?;
        let v = s.query_map(params![wing], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }

    /// Build an FTS5 MATCH expression that uses OR between individual terms for
    /// maximum recall. Single-quoted tokens handle special characters safely.
    fn build_fts_query(query: &str) -> String {
        let terms: Vec<String> = query
            .split_whitespace()
            .filter(|t| t.len() >= 2)
            .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
            .collect();
        if terms.is_empty() {
            // Fall back to a wildcard that FTS5 accepts for "match all"
            return "\"\"".to_string();
        }
        terms.join(" OR ")
    }

    fn map_fts_row(row: &rusqlite::Row) -> rusqlite::Result<(DrawerMetadata, String, f64)> {
        Ok((
            DrawerMetadata {
                drawer_id:    row.get(0)?,
                wing:         row.get(1)?,
                room:         row.get(2)?,
                source_file:  row.get(3)?,
                source_mtime: row.get(4)?,
                chunk_index:  row.get(5)?,
                type_:        row.get(6)?,
                created_at:   row.get(7)?,
                content_hash: row.get(8)?,
            },
            row.get::<_, String>(9)?,
            row.get::<_, f64>(10)?,
        ))
    }

    pub fn fts_search(
        &self,
        query: &str,
        wing: Option<&str>,
        room: Option<&str>,
        limit: i64,
    ) -> Result<Vec<(DrawerMetadata, String, f64)>, MpError> {
        let fts_match = Self::build_fts_query(query);
        let base_select = "SELECT d.drawer_id, d.wing, d.room, d.source_file, d.source_mtime, \
                                  d.chunk_index, d.type, d.created_at, d.content_hash, d.content, \
                                  bm25(drawers_fts) as score \
                           FROM drawers_fts \
                           JOIN drawers d ON drawers_fts.rowid = d.rowid";

        let rows = match (wing, room) {
            (Some(w), Some(r)) => {
                let sql = format!("{base_select} WHERE drawers_fts MATCH ?1 AND d.wing = ?2 AND d.room = ?3 ORDER BY score LIMIT ?4");
                let mut stmt = self.conn.prepare(&sql)?;
                let v = stmt.query_map(params![fts_match, w, r, limit], Self::map_fts_row)?
                    .collect::<Result<Vec<_>, _>>()?;
                v
            }
            (Some(w), None) => {
                let sql = format!("{base_select} WHERE drawers_fts MATCH ?1 AND d.wing = ?2 ORDER BY score LIMIT ?3");
                let mut stmt = self.conn.prepare(&sql)?;
                let v = stmt.query_map(params![fts_match, w, limit], Self::map_fts_row)?
                    .collect::<Result<Vec<_>, _>>()?;
                v
            }
            (None, Some(r)) => {
                let sql = format!("{base_select} WHERE drawers_fts MATCH ?1 AND d.room = ?2 ORDER BY score LIMIT ?3");
                let mut stmt = self.conn.prepare(&sql)?;
                let v = stmt.query_map(params![fts_match, r, limit], Self::map_fts_row)?
                    .collect::<Result<Vec<_>, _>>()?;
                v
            }
            (None, None) => {
                let sql = format!("{base_select} WHERE drawers_fts MATCH ?1 ORDER BY score LIMIT ?2");
                let mut stmt = self.conn.prepare(&sql)?;
                let v = stmt.query_map(params![fts_match, limit], Self::map_fts_row)?
                    .collect::<Result<Vec<_>, _>>()?;
                v
            }
        };
        Ok(rows)
    }

    pub fn get_all_embeddings(&self) -> Result<Vec<(String, Vec<f32>)>, MpError> {
        let mut s = self.conn.prepare(
            "SELECT drawer_id, embedding FROM drawers WHERE embedding IS NOT NULL"
        )?;
        let rows = s.query_map([], |row| {
            let id: String = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            Ok((id, bytes))
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows.into_iter().map(|(id, bytes)| {
            let emb = bytes.chunks(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            (id, emb)
        }).collect())
    }

    pub fn file_already_mined(&self, source_file: &str, mtime: Option<f64>) -> Result<bool, MpError> {
        let count: i64 = match mtime {
            Some(m) => self.conn.query_row(
                "SELECT COUNT(*) FROM drawers WHERE source_file = ?1 AND source_mtime = ?2",
                params![source_file, m],
                |r| r.get(0),
            )?,
            None => self.conn.query_row(
                "SELECT COUNT(*) FROM drawers WHERE source_file = ?1",
                params![source_file],
                |r| r.get(0),
            )?,
        };
        Ok(count > 0)
    }

    pub fn delete_file_drawers(&self, source_file: &str) -> Result<i64, MpError> {
        let n = self.conn.execute(
            "DELETE FROM drawers WHERE source_file = ?1",
            params![source_file],
        )?;
        Ok(n as i64)
    }

    pub fn get_stats(&self) -> Result<serde_json::Value, MpError> {
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM drawers", [], |r| r.get(0)
        )?;
        let wings = self.list_wings()?;
        let mut wing_counts = serde_json::Map::new();
        for w in &wings {
            let c = self.count_drawers(Some(w))?;
            wing_counts.insert(w.clone(), serde_json::Value::Number(c.into()));
        }
        Ok(serde_json::json!({
            "total_drawers": total,
            "wing_counts": wing_counts,
            "wings": wings,
        }))
    }

    pub fn create_tunnel(&self, tunnel: &TunnelRecord) -> Result<(), MpError> {
        self.conn.execute(
            r#"INSERT OR REPLACE INTO tunnels
               (tunnel_id, from_wing, from_room, to_wing, to_room, created_at, note)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            params![
                tunnel.tunnel_id, tunnel.from_wing, tunnel.from_room,
                tunnel.to_wing, tunnel.to_room, tunnel.created_at, tunnel.note
            ],
        )?;
        Ok(())
    }

    pub fn list_tunnels(&self, wing: Option<&str>) -> Result<Vec<TunnelRecord>, MpError> {
        let base = "SELECT tunnel_id, from_wing, from_room, to_wing, to_room, created_at, note \
                    FROM tunnels";
        match wing {
            Some(w) => {
                let sql = format!("{base} WHERE from_wing = ?1 OR to_wing = ?1");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![w], Self::row_to_tunnel)?.collect::<Result<Vec<_>, _>>()?;
                Ok(v)
            }
            None => {
                let mut s = self.conn.prepare(base)?;
                let v = s.query_map([], Self::row_to_tunnel)?.collect::<Result<Vec<_>, _>>()?;
                Ok(v)
            }
        }
    }

    fn row_to_tunnel(row: &rusqlite::Row) -> rusqlite::Result<TunnelRecord> {
        Ok(TunnelRecord {
            tunnel_id:  row.get(0)?,
            from_wing:  row.get(1)?,
            from_room:  row.get(2)?,
            to_wing:    row.get(3)?,
            to_room:    row.get(4)?,
            created_at: row.get(5)?,
            note:       row.get(6)?,
        })
    }

    pub fn delete_tunnel(&self, tunnel_id: &str) -> Result<bool, MpError> {
        let n = self.conn.execute(
            "DELETE FROM tunnels WHERE tunnel_id = ?1",
            params![tunnel_id],
        )?;
        Ok(n > 0)
    }
}
