use crate::error::MpError;
use crate::storage::{PalaceStore, DrawerMetadata};
use crate::storage::embedder::{Embedder, cosine_distance};
use serde::{Deserialize, Serialize};
use super::bm25::bm25_scores;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub drawer_id: String,
    pub wing: String,
    pub room: String,
    pub content: String,
    pub distance: f32,
    pub score: f64,
    pub source_file: Option<String>,
    pub type_: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMemoriesResult {
    pub results: Vec<SearchResult>,
    pub query: String,
    pub total_searched: usize,
    pub vector_enabled: bool,
}

pub fn search_memories(
    query: &str,
    store: &PalaceStore,
    embedder: Option<&Embedder>,
    wing: Option<&str>,
    room: Option<&str>,
    n_results: usize,
    max_distance: f32,
    vector_disabled: bool,
) -> Result<SearchMemoriesResult, MpError> {
    let n_results = n_results.clamp(1, 100);
    let max_distance = if max_distance <= 0.0 || max_distance > 2.0 {
        1.5
    } else {
        max_distance
    };
    let fts_limit = (n_results * 5).max(50) as i64;

    let fts_results = store.fts_search(query, wing, room, fts_limit).unwrap_or_default();

    let candidates: Vec<(DrawerMetadata, String, f64)> = if fts_results.is_empty() {
        store.list_drawers_with_content(wing, room, fts_limit)
            .unwrap_or_default()
            .into_iter()
            .map(|(m, content)| (m, content, 0.0))
            .collect()
    } else {
        fts_results
    };

    let total_searched = candidates.len();
    let use_vector = !vector_disabled
        && embedder.map(|e| e.is_available()).unwrap_or(false);

    if !use_vector || candidates.is_empty() {
        let texts: Vec<&str> = candidates.iter().map(|(_, c, _)| c.as_str()).collect();
        let bm25 = bm25_scores(query, &texts);

        let mut results: Vec<SearchResult> = candidates
            .into_iter()
            .enumerate()
            .map(|(i, (meta, content, fts_score))| {
                let score = if fts_score.abs() > 0.0 { fts_score.abs() } else { bm25[i] };
                SearchResult {
                    drawer_id: meta.drawer_id,
                    wing: meta.wing,
                    room: meta.room,
                    content,
                    distance: 1.0,
                    score,
                    source_file: meta.source_file,
                    type_: meta.type_,
                }
            })
            .filter(|r| r.distance <= max_distance)
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(n_results);

        return Ok(SearchMemoriesResult {
            results,
            query: query.to_string(),
            total_searched,
            vector_enabled: false,
        });
    }

    // SAFETY: `use_vector` is only true when `embedder.map(|e| e.is_available())` is Some(true)
    let embedder = embedder.expect("embedder verified as Some by use_vector check above");
    let query_embedding = embedder.embed(&[query])?;
    let query_vec = &query_embedding[0];

    let all_embeddings = store.get_all_embeddings().unwrap_or_default();
    let embedding_map: std::collections::HashMap<String, Vec<f32>> =
        all_embeddings.into_iter().collect();

    let texts: Vec<&str> = candidates.iter().map(|(_, c, _)| c.as_str()).collect();
    let bm25 = bm25_scores(query, &texts);
    let max_bm25 = bm25.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let max_bm25 = if max_bm25 > 0.0 { max_bm25 } else { 1.0 };

    let mut results: Vec<SearchResult> = candidates
        .into_iter()
        .enumerate()
        .map(|(i, (meta, content, _))| {
            let vec_dist = embedding_map
                .get(&meta.drawer_id)
                .map(|emb| cosine_distance(query_vec, emb))
                .unwrap_or(1.0);
            let vec_score = 1.0 - (vec_dist as f64 / 2.0);
            let bm25_norm = bm25[i] / max_bm25;
            let score = 0.6 * vec_score + 0.4 * bm25_norm;
            SearchResult {
                drawer_id: meta.drawer_id,
                wing: meta.wing,
                room: meta.room,
                content,
                distance: vec_dist,
                score,
                source_file: meta.source_file,
                type_: meta.type_,
            }
        })
        .filter(|r| r.distance <= max_distance)
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(n_results);

    Ok(SearchMemoriesResult {
        results,
        query: query.to_string(),
        total_searched,
        vector_enabled: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::redb_store::RedbPalaceStore;

    fn temp_store() -> PalaceStore {
        let tmp = tempfile::tempdir().expect("tempdir");
        RedbPalaceStore::open(tmp.path().to_str().expect("path")).expect("open")
    }

    #[test]
    fn search_bm25_only_no_embedder() {
        let store = temp_store();
        store.upsert_drawer("w", "r", "rust programming language", "text", None, None, None, None).unwrap();
        store.upsert_drawer("w", "r", "python scripting", "text", None, None, None, None).unwrap();

        let result = search_memories("rust", &store, None, None, None, 5, 1.5, false).unwrap();
        assert!(!result.results.is_empty());
        assert!(!result.vector_enabled);
    }

    #[test]
    fn search_vector_disabled_flag() {
        let store = temp_store();
        store.upsert_drawer("w", "r", "some content here", "text", None, None, None, None).unwrap();

        let result = search_memories("content", &store, None, None, None, 5, 1.5, true).unwrap();
        assert!(!result.vector_enabled);
    }

    #[test]
    fn search_empty_store() {
        let store = temp_store();
        let result = search_memories("anything", &store, None, None, None, 5, 1.5, false).unwrap();
        assert!(result.results.is_empty());
    }

    #[test]
    fn search_respects_n_results_cap() {
        let store = temp_store();
        for i in 0..20 {
            store.upsert_drawer("w", "r", &format!("document number {i} about rust"), "text", None, None, None, None).unwrap();
        }

        let result = search_memories("rust", &store, None, None, None, 3, 1.5, false).unwrap();
        assert!(result.results.len() <= 3);
    }

    #[test]
    fn search_filters_by_wing() {
        let store = temp_store();
        store.upsert_drawer("wing_a", "r", "rust programming", "text", None, None, None, None).unwrap();
        store.upsert_drawer("wing_b", "r", "rust language", "text", None, None, None, None).unwrap();

        let result = search_memories("rust", &store, None, Some("wing_a"), None, 10, 1.5, false).unwrap();
        for r in &result.results {
            assert_eq!(r.wing, "wing_a");
        }
    }

    #[test]
    fn search_clamps_n_results() {
        let store = temp_store();
        store.upsert_drawer("w", "r", "test content", "text", None, None, None, None).unwrap();

        // n_results=0 should be clamped to 1
        let result = search_memories("test", &store, None, None, None, 0, 1.5, false).unwrap();
        assert!(result.results.len() <= 1);
    }
}
