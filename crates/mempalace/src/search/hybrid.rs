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
    let n_results = n_results.max(1).min(100);
    let max_distance = if max_distance <= 0.0 || max_distance > 2.0 {
        1.5
    } else {
        max_distance
    };
    let fts_limit = (n_results * 5).max(50) as i64;

    let fts_results = store.fts_search(query, wing, room, fts_limit).unwrap_or_default();

    let candidates: Vec<(DrawerMetadata, String, f64)> = if fts_results.is_empty() {
        store.list_drawers(wing, room, Some(fts_limit))
            .unwrap_or_default()
            .into_iter()
            .map(|m| {
                let content = store.get_drawer(&m.drawer_id)
                    .map(|d| d.content)
                    .unwrap_or_default();
                (m, content, 0.0)
            })
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

    let embedder = embedder.unwrap();
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
