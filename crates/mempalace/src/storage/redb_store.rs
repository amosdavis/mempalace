use crate::error::MpError;
use crate::storage::sqlite::{DrawerMetadata, Drawer, TunnelRecord};
use chrono::Utc;
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;

const DRAWERS: TableDefinition<&str, &[u8]> = TableDefinition::new("drawers");
const DRAWERS_BY_WING: TableDefinition<&str, &str> = TableDefinition::new("drawers_by_wing");
const DRAWERS_BY_ROOM: TableDefinition<&str, &str> = TableDefinition::new("drawers_by_room");
const DRAWERS_BY_WING_ROOM: TableDefinition<&str, &str> =
    TableDefinition::new("drawers_by_wing_room");
const DRAWERS_BY_SOURCE: TableDefinition<&str, &str> = TableDefinition::new("drawers_by_source");
const EMBEDDINGS: TableDefinition<&str, &[u8]> = TableDefinition::new("embeddings");
const TUNNELS: TableDefinition<&str, &[u8]> = TableDefinition::new("tunnels");
const FTS_TERMS: TableDefinition<&str, &[u8]> = TableDefinition::new("fts_terms");

#[derive(Clone)]
pub struct RedbPalaceStore {
    db: Arc<Database>,
}

unsafe impl Send for RedbPalaceStore {}
unsafe impl Sync for RedbPalaceStore {}

impl RedbPalaceStore {
    pub fn open(palace_path: &str) -> Result<Self, MpError> {
        std::fs::create_dir_all(palace_path)?;
        let db_path = Path::new(palace_path).join("palace.redb");
        let db = Database::create(&db_path).map_err(|e| {
            MpError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;

        {
            let write_txn = db.begin_write().map_err(map_redb_err)?;
            write_txn.open_table(DRAWERS).map_err(map_redb_err)?;
            write_txn.open_table(DRAWERS_BY_WING).map_err(map_redb_err)?;
            write_txn.open_table(DRAWERS_BY_ROOM).map_err(map_redb_err)?;
            write_txn.open_table(DRAWERS_BY_WING_ROOM).map_err(map_redb_err)?;
            write_txn.open_table(DRAWERS_BY_SOURCE).map_err(map_redb_err)?;
            write_txn.open_table(EMBEDDINGS).map_err(map_redb_err)?;
            write_txn.open_table(TUNNELS).map_err(map_redb_err)?;
            write_txn.open_table(FTS_TERMS).map_err(map_redb_err)?;
            write_txn.commit().map_err(map_redb_err)?;
        }

        Ok(Self { db: Arc::new(db) })
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

        let record = DrawerRecord {
            drawer_id: drawer_id.clone(),
            wing: wing.to_string(),
            room: room.to_string(),
            source_file: source_file.map(|s| s.to_string()),
            source_mtime,
            chunk_index,
            type_: type_.to_string(),
            created_at,
            content_hash,
            content: content.to_string(),
        };
        let record_bytes = serde_json::to_vec(&record)?;

        let write_txn = self.db.begin_write().map_err(map_redb_err)?;
        {
            let mut drawers = write_txn.open_table(DRAWERS).map_err(map_redb_err)?;
            drawers
                .insert(drawer_id.as_str(), record_bytes.as_slice())
                .map_err(map_redb_err)?;

            let mut by_wing = write_txn.open_table(DRAWERS_BY_WING).map_err(map_redb_err)?;
            let wing_key = format!("{}:{}", wing, &drawer_id);
            by_wing
                .insert(wing_key.as_str(), drawer_id.as_str())
                .map_err(map_redb_err)?;

            let mut by_room = write_txn.open_table(DRAWERS_BY_ROOM).map_err(map_redb_err)?;
            let room_key = format!("{}:{}", room, &drawer_id);
            by_room
                .insert(room_key.as_str(), drawer_id.as_str())
                .map_err(map_redb_err)?;

            let mut by_wing_room =
                write_txn.open_table(DRAWERS_BY_WING_ROOM).map_err(map_redb_err)?;
            let wr_key = format!("{}:{}:{}", wing, room, &drawer_id);
            by_wing_room
                .insert(wr_key.as_str(), drawer_id.as_str())
                .map_err(map_redb_err)?;

            if let Some(sf) = source_file {
                let mut by_source =
                    write_txn.open_table(DRAWERS_BY_SOURCE).map_err(map_redb_err)?;
                let src_key = format!("{}:{}", sf, &drawer_id);
                by_source
                    .insert(src_key.as_str(), drawer_id.as_str())
                    .map_err(map_redb_err)?;
            }

            if let Some(emb) = embedding {
                let mut embeddings = write_txn.open_table(EMBEDDINGS).map_err(map_redb_err)?;
                let emb_bytes: Vec<u8> = emb.iter().flat_map(|f| f.to_le_bytes()).collect();
                embeddings
                    .insert(drawer_id.as_str(), emb_bytes.as_slice())
                    .map_err(map_redb_err)?;
            }

            self.index_content(&write_txn, &drawer_id, content)?;
        }
        write_txn.commit().map_err(map_redb_err)?;

        Ok(drawer_id)
    }

    pub fn get_drawer(&self, drawer_id: &str) -> Result<Drawer, MpError> {
        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        let table = read_txn.open_table(DRAWERS).map_err(map_redb_err)?;
        let value = table.get(drawer_id).map_err(map_redb_err)?;
        match value {
            Some(bytes) => {
                let record: DrawerRecord = serde_json::from_slice(bytes.value())?;
                Ok(record.into_drawer())
            }
            None => Err(MpError::NotFound(format!("Drawer {drawer_id} not found"))),
        }
    }

    pub fn list_drawers(
        &self,
        wing: Option<&str>,
        room: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<DrawerMetadata>, MpError> {
        let limit = limit.unwrap_or(100) as usize;
        let ids = self.query_drawer_ids(wing, room, limit)?;

        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        let table = read_txn.open_table(DRAWERS).map_err(map_redb_err)?;

        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(bytes) = table.get(id.as_str()).map_err(map_redb_err)? {
                let record: DrawerRecord = serde_json::from_slice(bytes.value())?;
                results.push(record.into_metadata());
            }
        }
        Ok(results)
    }

    pub fn list_drawers_with_content(
        &self,
        wing: Option<&str>,
        room: Option<&str>,
        limit: i64,
    ) -> Result<Vec<(DrawerMetadata, String)>, MpError> {
        let ids = self.query_drawer_ids(wing, room, limit as usize)?;

        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        let table = read_txn.open_table(DRAWERS).map_err(map_redb_err)?;

        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(bytes) = table.get(id.as_str()).map_err(map_redb_err)? {
                let record: DrawerRecord = serde_json::from_slice(bytes.value())?;
                let content = record.content.clone();
                results.push((record.into_metadata(), content));
            }
        }
        Ok(results)
    }

    pub fn delete_drawer(&self, drawer_id: &str) -> Result<bool, MpError> {
        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        let table = read_txn.open_table(DRAWERS).map_err(map_redb_err)?;
        let record = match table.get(drawer_id).map_err(map_redb_err)? {
            Some(bytes) => {
                let r: DrawerRecord = serde_json::from_slice(bytes.value())?;
                r
            }
            None => return Ok(false),
        };
        drop(table);
        drop(read_txn);

        let write_txn = self.db.begin_write().map_err(map_redb_err)?;
        {
            let mut drawers = write_txn.open_table(DRAWERS).map_err(map_redb_err)?;
            drawers.remove(drawer_id).map_err(map_redb_err)?;

            let mut by_wing = write_txn.open_table(DRAWERS_BY_WING).map_err(map_redb_err)?;
            let wing_key = format!("{}:{}", record.wing, drawer_id);
            by_wing.remove(wing_key.as_str()).map_err(map_redb_err)?;

            let mut by_room = write_txn.open_table(DRAWERS_BY_ROOM).map_err(map_redb_err)?;
            let room_key = format!("{}:{}", record.room, drawer_id);
            by_room.remove(room_key.as_str()).map_err(map_redb_err)?;

            let mut by_wr = write_txn.open_table(DRAWERS_BY_WING_ROOM).map_err(map_redb_err)?;
            let wr_key = format!("{}:{}:{}", record.wing, record.room, drawer_id);
            by_wr.remove(wr_key.as_str()).map_err(map_redb_err)?;

            if let Some(ref sf) = record.source_file {
                let mut by_source =
                    write_txn.open_table(DRAWERS_BY_SOURCE).map_err(map_redb_err)?;
                let src_key = format!("{}:{}", sf, drawer_id);
                by_source.remove(src_key.as_str()).map_err(map_redb_err)?;
            }

            let mut embeddings = write_txn.open_table(EMBEDDINGS).map_err(map_redb_err)?;
            let _ = embeddings.remove(drawer_id).map_err(map_redb_err)?;

            self.remove_from_index(&write_txn, drawer_id, &record.content)?;
        }
        write_txn.commit().map_err(map_redb_err)?;
        Ok(true)
    }

    pub fn count_drawers(&self, wing: Option<&str>) -> Result<i64, MpError> {
        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        match wing {
            Some(w) => {
                let table = read_txn.open_table(DRAWERS_BY_WING).map_err(map_redb_err)?;
                let prefix = format!("{}:", w);
                let count = table
                    .range(prefix.as_str()..)
                    .map_err(map_redb_err)?
                    .take_while(|r| {
                        r.as_ref()
                            .map(|(k, _)| k.value().starts_with(&prefix))
                            .unwrap_or(false)
                    })
                    .count();
                Ok(count as i64)
            }
            None => {
                let table = read_txn.open_table(DRAWERS).map_err(map_redb_err)?;
                Ok(table.len().map_err(map_redb_err)? as i64)
            }
        }
    }

    pub fn list_wings(&self) -> Result<Vec<String>, MpError> {
        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        let table = read_txn.open_table(DRAWERS_BY_WING).map_err(map_redb_err)?;

        let mut wings: Vec<String> = Vec::new();
        let mut last_wing = String::new();
        for item in table.iter().map_err(map_redb_err)? {
            let (key, _) = item.map_err(map_redb_err)?;
            let k = key.value();
            if let Some(colon) = k.find(':') {
                let wing = &k[..colon];
                if wing != last_wing {
                    wings.push(wing.to_string());
                    last_wing = wing.to_string();
                }
            }
        }
        wings.sort();
        wings.dedup();
        Ok(wings)
    }

    pub fn list_rooms(&self, wing: &str) -> Result<Vec<String>, MpError> {
        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        let table = read_txn.open_table(DRAWERS_BY_WING_ROOM).map_err(map_redb_err)?;

        let prefix = format!("{}:", wing);
        let mut rooms: Vec<String> = Vec::new();
        for item in table.range(prefix.as_str()..).map_err(map_redb_err)? {
            let (key, _) = item.map_err(map_redb_err)?;
            let k = key.value();
            if !k.starts_with(&prefix) {
                break;
            }
            let rest = &k[prefix.len()..];
            if let Some(colon) = rest.find(':') {
                let room = &rest[..colon];
                if rooms.last().map(|r| r.as_str()) != Some(room) {
                    rooms.push(room.to_string());
                }
            }
        }
        rooms.sort();
        rooms.dedup();
        Ok(rooms)
    }

    pub fn fts_search(
        &self,
        query: &str,
        wing: Option<&str>,
        room: Option<&str>,
        limit: i64,
    ) -> Result<Vec<(DrawerMetadata, String, f64)>, MpError> {
        let terms: Vec<&str> = query
            .split_whitespace()
            .filter(|t| t.len() >= 2)
            .collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        let fts_table = read_txn.open_table(FTS_TERMS).map_err(map_redb_err)?;

        let mut doc_scores: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        for term in &terms {
            let term_lower = term.to_lowercase();
            if let Some(bytes) = fts_table.get(term_lower.as_str()).map_err(map_redb_err)? {
                let ids: Vec<String> = serde_json::from_slice(bytes.value()).unwrap_or_default();
                for id in ids {
                    *doc_scores.entry(id).or_insert(0.0) += 1.0;
                }
            }
        }

        let mut scored: Vec<(String, f64)> = doc_scores.into_iter().collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let drawers_table = read_txn.open_table(DRAWERS).map_err(map_redb_err)?;
        let mut results = Vec::new();
        for (id, score) in scored {
            if results.len() >= limit as usize {
                break;
            }
            if let Some(bytes) = drawers_table.get(id.as_str()).map_err(map_redb_err)? {
                let record: DrawerRecord = serde_json::from_slice(bytes.value())?;
                if let Some(w) = wing {
                    if record.wing != w {
                        continue;
                    }
                }
                if let Some(r) = room {
                    if record.room != r {
                        continue;
                    }
                }
                let content = record.content.clone();
                results.push((record.into_metadata(), content, score));
            }
        }
        Ok(results)
    }

    pub fn get_all_embeddings(&self) -> Result<Vec<(String, Vec<f32>)>, MpError> {
        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        let table = read_txn.open_table(EMBEDDINGS).map_err(map_redb_err)?;

        let mut results = Vec::new();
        for item in table.iter().map_err(map_redb_err)? {
            let (key, value) = item.map_err(map_redb_err)?;
            let id = key.value().to_string();
            let bytes = value.value();
            let emb: Vec<f32> = bytes
                .chunks(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            results.push((id, emb));
        }
        Ok(results)
    }

    pub fn file_already_mined(&self, source_file: &str, mtime: Option<f64>) -> Result<bool, MpError> {
        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        let table = read_txn.open_table(DRAWERS_BY_SOURCE).map_err(map_redb_err)?;

        let prefix = format!("{}:", source_file);
        let has_entries = table
            .range(prefix.as_str()..)
            .map_err(map_redb_err)?
            .take_while(|r| {
                r.as_ref()
                    .map(|(k, _)| k.value().starts_with(&prefix))
                    .unwrap_or(false)
            })
            .next()
            .is_some();

        if !has_entries {
            return Ok(false);
        }

        if let Some(expected_mtime) = mtime {
            let drawers_table = read_txn.open_table(DRAWERS).map_err(map_redb_err)?;
            for item in table.range(prefix.as_str()..).map_err(map_redb_err)? {
                let (key, value) = item.map_err(map_redb_err)?;
                if !key.value().starts_with(&prefix) {
                    break;
                }
                let drawer_id = value.value();
                if let Some(bytes) = drawers_table.get(drawer_id).map_err(map_redb_err)? {
                    let record: DrawerRecord = serde_json::from_slice(bytes.value())?;
                    if record.source_mtime == Some(expected_mtime) {
                        return Ok(true);
                    }
                }
                break;
            }
            return Ok(false);
        }
        Ok(true)
    }

    pub fn delete_file_drawers(&self, source_file: &str) -> Result<i64, MpError> {
        let ids_to_delete = {
            let read_txn = self.db.begin_read().map_err(map_redb_err)?;
            let table = read_txn.open_table(DRAWERS_BY_SOURCE).map_err(map_redb_err)?;
            let prefix = format!("{}:", source_file);
            let mut ids = Vec::new();
            for item in table.range(prefix.as_str()..).map_err(map_redb_err)? {
                let (key, value) = item.map_err(map_redb_err)?;
                if !key.value().starts_with(&prefix) {
                    break;
                }
                ids.push(value.value().to_string());
            }
            ids
        };

        let count = ids_to_delete.len() as i64;
        for id in &ids_to_delete {
            self.delete_drawer(id)?;
        }
        Ok(count)
    }

    pub fn replace_file_drawers(
        &self,
        wing: &str,
        room: &str,
        source_file: &str,
        chunks: &[(&str, Option<&[f32]>)],
        source_mtime: Option<f64>,
    ) -> Result<(usize, Vec<(String, Vec<String>)>), MpError> {
        let ids_to_delete = {
            let read_txn = self.db.begin_read().map_err(map_redb_err)?;
            let table = read_txn.open_table(DRAWERS_BY_SOURCE).map_err(map_redb_err)?;
            let prefix = format!("{}:", source_file);
            let mut ids = Vec::new();
            for item in table.range(prefix.as_str()..).map_err(map_redb_err)? {
                let (key, value) = item.map_err(map_redb_err)?;
                if !key.value().starts_with(&prefix) {
                    break;
                }
                ids.push(value.value().to_string());
            }
            ids
        };

        let old_drawer_ids: Vec<String> = ids_to_delete.clone();

        let write_txn = self.db.begin_write().map_err(map_redb_err)?;
        let mut new_entries: Vec<(String, Vec<String>)> = Vec::new();
        {
            let mut drawers = write_txn.open_table(DRAWERS).map_err(map_redb_err)?;
            let mut by_wing = write_txn.open_table(DRAWERS_BY_WING).map_err(map_redb_err)?;
            let mut by_room = write_txn.open_table(DRAWERS_BY_ROOM).map_err(map_redb_err)?;
            let mut by_wr = write_txn.open_table(DRAWERS_BY_WING_ROOM).map_err(map_redb_err)?;
            let mut by_source = write_txn.open_table(DRAWERS_BY_SOURCE).map_err(map_redb_err)?;
            let mut embeddings_tbl = write_txn.open_table(EMBEDDINGS).map_err(map_redb_err)?;

            for id in &old_drawer_ids {
                drawers.remove(id.as_str()).map_err(map_redb_err)?;
                let wing_key = format!("{}:{}", wing, id);
                by_wing.remove(wing_key.as_str()).map_err(map_redb_err)?;
                let room_key = format!("{}:{}", room, id);
                by_room.remove(room_key.as_str()).map_err(map_redb_err)?;
                let wr_key = format!("{}:{}:{}", wing, room, id);
                by_wr.remove(wr_key.as_str()).map_err(map_redb_err)?;
                let src_key = format!("{}:{}", source_file, id);
                by_source.remove(src_key.as_str()).map_err(map_redb_err)?;
                let _ = embeddings_tbl.remove(id.as_str());
            }

            let created_at = Utc::now().timestamp_millis() as f64 / 1000.0;
            for (i, (content, embedding)) in chunks.iter().enumerate() {
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

                let record = DrawerRecord {
                    drawer_id: drawer_id.clone(),
                    wing: wing.to_string(),
                    room: room.to_string(),
                    source_file: Some(source_file.to_string()),
                    source_mtime,
                    chunk_index: Some(i as i64),
                    type_: "code".to_string(),
                    created_at,
                    content_hash,
                    content: content.to_string(),
                };
                let record_bytes = serde_json::to_vec(&record)?;

                drawers.insert(drawer_id.as_str(), record_bytes.as_slice()).map_err(map_redb_err)?;

                let wing_key = format!("{}:{}", wing, &drawer_id);
                by_wing.insert(wing_key.as_str(), drawer_id.as_str()).map_err(map_redb_err)?;

                let room_key = format!("{}:{}", room, &drawer_id);
                by_room.insert(room_key.as_str(), drawer_id.as_str()).map_err(map_redb_err)?;

                let wr_key = format!("{}:{}:{}", wing, room, &drawer_id);
                by_wr.insert(wr_key.as_str(), drawer_id.as_str()).map_err(map_redb_err)?;

                let src_key = format!("{}:{}", source_file, &drawer_id);
                by_source.insert(src_key.as_str(), drawer_id.as_str()).map_err(map_redb_err)?;

                if let Some(emb) = embedding {
                    let emb_bytes: Vec<u8> = emb.iter().flat_map(|f| f.to_le_bytes()).collect();
                    embeddings_tbl.insert(drawer_id.as_str(), emb_bytes.as_slice()).map_err(map_redb_err)?;
                }

                let terms = tokenize(content);
                new_entries.push((drawer_id, terms));
            }
        }
        write_txn.commit().map_err(map_redb_err)?;
        Ok((chunks.len(), new_entries))
    }

    pub fn flush_fts_index(&self, index: &std::collections::HashMap<String, Vec<String>>) -> Result<usize, MpError> {
        let total = index.len();
        let entries: Vec<(&String, &Vec<String>)> = index.iter().collect();
        let batch_size = 2000;
        for batch in entries.chunks(batch_size) {
            let write_txn = self.db.begin_write().map_err(map_redb_err)?;
            {
                let mut fts = write_txn.open_table(FTS_TERMS).map_err(map_redb_err)?;
                for (term, new_ids) in batch {
                    let existing: Vec<String> = fts.get(term.as_str())
                        .map_err(map_redb_err)?
                        .and_then(|v| serde_json::from_slice(v.value()).ok())
                        .unwrap_or_default();
                    let mut merged = existing;
                    for id in *new_ids {
                        if !merged.contains(id) {
                            merged.push(id.clone());
                        }
                    }
                    let bytes = serde_json::to_vec(&merged)?;
                    fts.insert(term.as_str(), bytes.as_slice()).map_err(map_redb_err)?;
                }
            }
            write_txn.commit().map_err(map_redb_err)?;
        }
        Ok(total)
    }

    pub fn get_stats(&self) -> Result<serde_json::Value, MpError> {
        let total = self.count_drawers(None)?;
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
        let bytes = serde_json::to_vec(tunnel)?;
        let write_txn = self.db.begin_write().map_err(map_redb_err)?;
        {
            let mut table = write_txn.open_table(TUNNELS).map_err(map_redb_err)?;
            table
                .insert(tunnel.tunnel_id.as_str(), bytes.as_slice())
                .map_err(map_redb_err)?;
        }
        write_txn.commit().map_err(map_redb_err)?;
        Ok(())
    }

    pub fn list_tunnels(&self, wing: Option<&str>) -> Result<Vec<TunnelRecord>, MpError> {
        let read_txn = self.db.begin_read().map_err(map_redb_err)?;
        let table = read_txn.open_table(TUNNELS).map_err(map_redb_err)?;

        let mut results = Vec::new();
        for item in table.iter().map_err(map_redb_err)? {
            let (_, value) = item.map_err(map_redb_err)?;
            let tunnel: TunnelRecord = serde_json::from_slice(value.value())?;
            if let Some(w) = wing {
                if tunnel.from_wing != w && tunnel.to_wing != w {
                    continue;
                }
            }
            results.push(tunnel);
        }
        Ok(results)
    }

    pub fn delete_tunnel(&self, tunnel_id: &str) -> Result<bool, MpError> {
        let write_txn = self.db.begin_write().map_err(map_redb_err)?;
        {
            let mut table = write_txn.open_table(TUNNELS).map_err(map_redb_err)?;
            let existed = table.remove(tunnel_id).map_err(map_redb_err)?.is_some();
            if !existed {
                return Ok(false);
            }
        }
        write_txn.commit().map_err(map_redb_err)?;
        Ok(true)
    }

    fn query_drawer_ids(
        &self,
        wing: Option<&str>,
        room: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, MpError> {
        let read_txn = self.db.begin_read().map_err(map_redb_err)?;

        match (wing, room) {
            (Some(w), Some(r)) => {
                let table = read_txn.open_table(DRAWERS_BY_WING_ROOM).map_err(map_redb_err)?;
                let prefix = format!("{}:{}:", w, r);
                let ids: Vec<String> = table
                    .range(prefix.as_str()..)
                    .map_err(map_redb_err)?
                    .take_while(|item| {
                        item.as_ref()
                            .map(|(k, _)| k.value().starts_with(&prefix))
                            .unwrap_or(false)
                    })
                    .filter_map(|item| item.ok().map(|(_, v)| v.value().to_string()))
                    .take(limit)
                    .collect();
                Ok(ids)
            }
            (Some(w), None) => {
                let table = read_txn.open_table(DRAWERS_BY_WING).map_err(map_redb_err)?;
                let prefix = format!("{}:", w);
                let ids: Vec<String> = table
                    .range(prefix.as_str()..)
                    .map_err(map_redb_err)?
                    .take_while(|item| {
                        item.as_ref()
                            .map(|(k, _)| k.value().starts_with(&prefix))
                            .unwrap_or(false)
                    })
                    .filter_map(|item| item.ok().map(|(_, v)| v.value().to_string()))
                    .take(limit)
                    .collect();
                Ok(ids)
            }
            (None, Some(r)) => {
                let table = read_txn.open_table(DRAWERS_BY_ROOM).map_err(map_redb_err)?;
                let prefix = format!("{}:", r);
                let ids: Vec<String> = table
                    .range(prefix.as_str()..)
                    .map_err(map_redb_err)?
                    .take_while(|item| {
                        item.as_ref()
                            .map(|(k, _)| k.value().starts_with(&prefix))
                            .unwrap_or(false)
                    })
                    .filter_map(|item| item.ok().map(|(_, v)| v.value().to_string()))
                    .take(limit)
                    .collect();
                Ok(ids)
            }
            (None, None) => {
                let table = read_txn.open_table(DRAWERS).map_err(map_redb_err)?;
                let ids: Vec<String> = table
                    .iter()
                    .map_err(map_redb_err)?
                    .filter_map(|item| item.ok().map(|(k, _)| k.value().to_string()))
                    .take(limit)
                    .collect();
                Ok(ids)
            }
        }
    }

    fn index_content(
        &self,
        write_txn: &redb::WriteTransaction,
        drawer_id: &str,
        content: &str,
    ) -> Result<(), MpError> {
        let mut fts = write_txn.open_table(FTS_TERMS).map_err(map_redb_err)?;
        let terms = tokenize(content);
        for term in terms {
            let mut ids: Vec<String> = {
                let existing = fts.get(term.as_str()).map_err(map_redb_err)?;
                match existing {
                    Some(bytes) => serde_json::from_slice(bytes.value()).unwrap_or_default(),
                    None => Vec::new(),
                }
            };
            if !ids.contains(&drawer_id.to_string()) {
                ids.push(drawer_id.to_string());
                let bytes = serde_json::to_vec(&ids)?;
                fts.insert(term.as_str(), bytes.as_slice())
                    .map_err(map_redb_err)?;
            }
        }
        Ok(())
    }

    fn remove_from_index(
        &self,
        write_txn: &redb::WriteTransaction,
        drawer_id: &str,
        content: &str,
    ) -> Result<(), MpError> {
        let mut fts = write_txn.open_table(FTS_TERMS).map_err(map_redb_err)?;
        let terms = tokenize(content);
        for term in terms {
            let ids_opt: Option<Vec<String>> = {
                let existing = fts.get(term.as_str()).map_err(map_redb_err)?;
                existing.map(|bytes| serde_json::from_slice(bytes.value()).unwrap_or_default())
            };
            if let Some(mut ids) = ids_opt {
                ids.retain(|id| id != drawer_id);
                if ids.is_empty() {
                    fts.remove(term.as_str()).map_err(map_redb_err)?;
                } else {
                    let bytes = serde_json::to_vec(&ids)?;
                    fts.insert(term.as_str(), bytes.as_slice())
                        .map_err(map_redb_err)?;
                }
            }
        }
        Ok(())
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| t.len() >= 2)
        .map(|t| t.to_lowercase())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

fn map_redb_err(e: impl std::fmt::Display) -> MpError {
    MpError::Io(std::io::Error::new(
        std::io::ErrorKind::Other,
        e.to_string(),
    ))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DrawerRecord {
    drawer_id: String,
    wing: String,
    room: String,
    source_file: Option<String>,
    source_mtime: Option<f64>,
    chunk_index: Option<i64>,
    type_: String,
    created_at: f64,
    content_hash: String,
    content: String,
}

impl DrawerRecord {
    fn into_metadata(self) -> DrawerMetadata {
        DrawerMetadata {
            drawer_id: self.drawer_id,
            wing: self.wing,
            room: self.room,
            source_file: self.source_file,
            source_mtime: self.source_mtime,
            chunk_index: self.chunk_index,
            type_: self.type_,
            created_at: self.created_at,
            content_hash: self.content_hash,
        }
    }

    fn into_drawer(self) -> Drawer {
        let content = self.content.clone();
        Drawer {
            metadata: self.into_metadata(),
            content,
        }
    }
}
