use crate::error::MpError;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub entity_id: String,
    pub name: String,
    pub entity_type: String,
    pub attributes: serde_json::Value,
    pub created_at: f64,
    pub updated_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Triple {
    pub triple_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub source: Option<String>,
    pub created_at: f64,
}

pub struct KnowledgeGraph {
    conn: Connection,
}

impl KnowledgeGraph {
    pub fn open(db_path: &std::path::Path) -> Result<Self, MpError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let kg = Self { conn };
        kg.create_schema()?;
        Ok(kg)
    }

    fn create_schema(&self) -> Result<(), MpError> {
        self.conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS entities (
                entity_id   TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                entity_type TEXT NOT NULL DEFAULT 'person',
                attributes  TEXT NOT NULL DEFAULT '{}',
                created_at  REAL NOT NULL,
                updated_at  REAL NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);

            CREATE TABLE IF NOT EXISTS triples (
                triple_id  TEXT PRIMARY KEY,
                subject    TEXT NOT NULL,
                predicate  TEXT NOT NULL,
                object     TEXT NOT NULL,
                valid_from TEXT,
                valid_to   TEXT,
                source     TEXT,
                created_at REAL NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_triples_subject ON triples(subject);
            CREATE INDEX IF NOT EXISTS idx_triples_object  ON triples(object);
            CREATE INDEX IF NOT EXISTS idx_triples_pred    ON triples(predicate);
        "#)?;
        Ok(())
    }

    pub fn entity_id(name: &str) -> String {
        name.to_lowercase()
            .chars()
            .map(|c| if matches!(c, ' ' | '\'' | '-') { '_' } else { c })
            .collect()
    }

    pub fn add_entity(
        &self,
        name: &str,
        entity_type: &str,
        attributes: &serde_json::Value,
    ) -> Result<String, MpError> {
        let entity_id = Self::entity_id(name);
        let now = Utc::now().timestamp_millis() as f64 / 1000.0;
        let attrs = serde_json::to_string(attributes)?;
        self.conn.execute(
            r#"INSERT INTO entities (entity_id, name, entity_type, attributes, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)
               ON CONFLICT(entity_id) DO UPDATE SET
                   name        = excluded.name,
                   entity_type = excluded.entity_type,
                   attributes  = excluded.attributes,
                   updated_at  = excluded.updated_at"#,
            params![entity_id, name, entity_type, attrs, now, now],
        )?;
        Ok(entity_id)
    }

    pub fn get_entity(&self, name: &str) -> Result<Option<Entity>, MpError> {
        let entity_id = Self::entity_id(name);
        match self.conn.query_row(
            "SELECT entity_id, name, entity_type, attributes, created_at, updated_at \
             FROM entities WHERE entity_id = ?1",
            params![entity_id],
            |row| {
                let attrs_str: String = row.get(3)?;
                Ok(Entity {
                    entity_id:   row.get(0)?,
                    name:        row.get(1)?,
                    entity_type: row.get(2)?,
                    attributes:  serde_json::from_str(&attrs_str)
                                     .unwrap_or(serde_json::Value::Object(Default::default())),
                    created_at:  row.get(4)?,
                    updated_at:  row.get(5)?,
                })
            },
        ) {
            Ok(e) => Ok(Some(e)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MpError::Storage(e)),
        }
    }

    pub fn list_entities(&self, limit: i64) -> Result<Vec<Entity>, MpError> {
        let mut s = self.conn.prepare(
            "SELECT entity_id, name, entity_type, attributes, created_at, updated_at \
             FROM entities ORDER BY updated_at DESC LIMIT ?1"
        )?;
        let v = s.query_map(params![limit], |row| {
            let attrs_str: String = row.get(3)?;
            Ok(Entity {
                entity_id:   row.get(0)?,
                name:        row.get(1)?,
                entity_type: row.get(2)?,
                attributes:  serde_json::from_str(&attrs_str)
                                 .unwrap_or(serde_json::Value::Object(Default::default())),
                created_at:  row.get(4)?,
                updated_at:  row.get(5)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }

    pub fn add_triple(
        &self,
        subject: &str,
        predicate: &str,
        object: &str,
        valid_from: Option<&str>,
        valid_to: Option<&str>,
        source: Option<&str>,
    ) -> Result<String, MpError> {
        let mut h = Sha256::new();
        h.update(subject.as_bytes());
        h.update(predicate.as_bytes());
        h.update(object.as_bytes());
        if let Some(vf) = valid_from { h.update(vf.as_bytes()); }
        let triple_id = hex::encode(h.finalize())[..16].to_string();
        let now = Utc::now().timestamp_millis() as f64 / 1000.0;
        self.conn.execute(
            r#"INSERT OR REPLACE INTO triples
               (triple_id, subject, predicate, object, valid_from, valid_to, source, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![triple_id, subject, predicate, object, valid_from, valid_to, source, now],
        )?;
        Ok(triple_id)
    }

    pub fn query_entity(
        &self,
        name: &str,
        as_of: Option<&str>,
        direction: &str,
    ) -> Result<Vec<Triple>, MpError> {
        let entity_id = Self::entity_id(name);
        let base = "SELECT triple_id, subject, predicate, object, valid_from, valid_to, \
                           source, created_at FROM triples";
        let rows: Vec<Triple> = match direction {
            "in" => {
                let sql = format!("{base} WHERE object = ?1 ORDER BY created_at DESC");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![entity_id], Self::row_to_triple)?
                         .collect::<Result<Vec<_>, _>>()?;
                v
            }
            "out" => {
                let sql = format!("{base} WHERE subject = ?1 ORDER BY created_at DESC");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![entity_id], Self::row_to_triple)?
                         .collect::<Result<Vec<_>, _>>()?;
                v
            }
            _ => {
                let sql = format!("{base} WHERE subject = ?1 OR object = ?1 ORDER BY created_at DESC");
                let mut s = self.conn.prepare(&sql)?;
                let v = s.query_map(params![entity_id], Self::row_to_triple)?
                         .collect::<Result<Vec<_>, _>>()?;
                v
            }
        };

        let rows = if let Some(date) = as_of {
            rows.into_iter()
                .filter(|t| {
                    t.valid_to.as_deref().map(|vt| vt >= date).unwrap_or(true)
                    && t.valid_from.as_deref().map(|vf| vf <= date).unwrap_or(true)
                })
                .collect()
        } else {
            rows
        };
        Ok(rows)
    }

    pub fn query_relationship(&self, predicate: &str, limit: i64) -> Result<Vec<Triple>, MpError> {
        let mut s = self.conn.prepare(
            "SELECT triple_id, subject, predicate, object, valid_from, valid_to, source, created_at \
             FROM triples WHERE predicate = ?1 ORDER BY created_at DESC LIMIT ?2"
        )?;
        let v = s.query_map(params![predicate, limit], Self::row_to_triple)?
                 .collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }

    pub fn invalidate_triple(&self, triple_id: &str, valid_to: &str) -> Result<bool, MpError> {
        let n = self.conn.execute(
            "UPDATE triples SET valid_to = ?1 WHERE triple_id = ?2",
            params![valid_to, triple_id],
        )?;
        Ok(n > 0)
    }

    pub fn timeline(&self, limit: i64) -> Result<Vec<Triple>, MpError> {
        let mut s = self.conn.prepare(
            "SELECT triple_id, subject, predicate, object, valid_from, valid_to, source, created_at \
             FROM triples ORDER BY created_at DESC LIMIT ?1"
        )?;
        let v = s.query_map(params![limit], Self::row_to_triple)?
                 .collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }

    pub fn stats(&self) -> Result<serde_json::Value, MpError> {
        let entities: i64 = self.conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
        let triples: i64  = self.conn.query_row("SELECT COUNT(*) FROM triples",  [], |r| r.get(0))?;
        Ok(serde_json::json!({ "entity_count": entities, "triple_count": triples }))
    }

    fn row_to_triple(row: &rusqlite::Row) -> rusqlite::Result<Triple> {
        Ok(Triple {
            triple_id:  row.get(0)?,
            subject:    row.get(1)?,
            predicate:  row.get(2)?,
            object:     row.get(3)?,
            valid_from: row.get(4)?,
            valid_to:   row.get(5)?,
            source:     row.get(6)?,
            created_at: row.get(7)?,
        })
    }
}
