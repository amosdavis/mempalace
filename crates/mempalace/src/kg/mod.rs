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
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_kg() -> (KnowledgeGraph, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db_path = tmp.path().join("test_kg.sqlite3");
        let kg = KnowledgeGraph::open(&db_path).expect("open");
        (kg, tmp)
    }

    #[test]
    fn add_and_get_entity() {
        let (kg, _tmp) = temp_kg();
        let id = kg.add_entity("Alice", "person", &serde_json::json!({"age": 30})).unwrap();
        assert_eq!(id, "alice");

        let entity = kg.get_entity("Alice").unwrap().expect("entity should exist");
        assert_eq!(entity.name, "Alice");
        assert_eq!(entity.entity_type, "person");
        assert_eq!(entity.attributes["age"], 30);
    }

    #[test]
    fn get_nonexistent_entity() {
        let (kg, _tmp) = temp_kg();
        assert!(kg.get_entity("nobody").unwrap().is_none());
    }

    #[test]
    fn add_and_query_triple() {
        let (kg, _tmp) = temp_kg();
        let tid = kg.add_triple("alice", "knows", "bob", None, None, None).unwrap();
        assert!(!tid.is_empty());

        let triples = kg.query_entity("alice", None, "out").unwrap();
        assert_eq!(triples.len(), 1);
        assert_eq!(triples[0].predicate, "knows");
        assert_eq!(triples[0].object, "bob");
    }

    #[test]
    fn query_entity_direction_in() {
        let (kg, _tmp) = temp_kg();
        kg.add_triple("alice", "knows", "bob", None, None, None).unwrap();

        let triples = kg.query_entity("bob", None, "in").unwrap();
        assert_eq!(triples.len(), 1);
        assert_eq!(triples[0].subject, "alice");
    }

    #[test]
    fn query_entity_direction_both() {
        let (kg, _tmp) = temp_kg();
        kg.add_triple("alice", "knows", "bob", None, None, None).unwrap();
        kg.add_triple("charlie", "knows", "alice", None, None, None).unwrap();

        let triples = kg.query_entity("alice", None, "both").unwrap();
        assert_eq!(triples.len(), 2);
    }

    #[test]
    fn invalidate_triple() {
        let (kg, _tmp) = temp_kg();
        let tid = kg.add_triple("alice", "lives_in", "nyc", Some("2020-01-01"), None, None).unwrap();
        assert!(kg.invalidate_triple(&tid, "2024-06-01").unwrap());

        let triples = kg.query_entity("alice", Some("2025-01-01"), "out").unwrap();
        assert!(triples.is_empty(), "invalidated triple should be filtered out");
    }

    #[test]
    fn timeline_ordering() {
        let (kg, _tmp) = temp_kg();
        kg.add_triple("a", "p1", "b", None, None, None).unwrap();
        kg.add_triple("c", "p2", "d", None, None, None).unwrap();

        let timeline = kg.timeline(10).unwrap();
        assert_eq!(timeline.len(), 2);
        // Most recent first
        assert!(timeline[0].created_at >= timeline[1].created_at);
    }

    #[test]
    fn stats_counts() {
        let (kg, _tmp) = temp_kg();
        kg.add_entity("alice", "person", &serde_json::json!({})).unwrap();
        kg.add_triple("alice", "knows", "bob", None, None, None).unwrap();

        let stats = kg.stats().unwrap();
        assert_eq!(stats["entity_count"], 1);
        assert_eq!(stats["triple_count"], 1);
    }

    #[test]
    fn list_entities() {
        let (kg, _tmp) = temp_kg();
        kg.add_entity("Alice", "person", &serde_json::json!({})).unwrap();
        kg.add_entity("Bob", "person", &serde_json::json!({})).unwrap();

        let entities = kg.list_entities(10).unwrap();
        assert_eq!(entities.len(), 2);
    }

    #[test]
    fn entity_id_normalization() {
        assert_eq!(KnowledgeGraph::entity_id("Alice O'Brien"), "alice_o_brien");
        assert_eq!(KnowledgeGraph::entity_id("foo-bar"), "foo_bar");
    }

    #[test]
    fn upsert_entity_updates() {
        let (kg, _tmp) = temp_kg();
        kg.add_entity("Alice", "person", &serde_json::json!({"age": 25})).unwrap();
        kg.add_entity("Alice", "person", &serde_json::json!({"age": 30})).unwrap();

        let entity = kg.get_entity("Alice").unwrap().unwrap();
        assert_eq!(entity.attributes["age"], 30);

        let entities = kg.list_entities(10).unwrap();
        assert_eq!(entities.len(), 1, "upsert should not create duplicate");
    }
}
