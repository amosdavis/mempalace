use crate::error::MpError;
use crate::mining::progress::{write_progress, MineProgress, MineStatus};
use crate::storage::{Embedder, PalaceStore};
use serde::Deserialize;
use std::path::Path;
use walkdir::WalkDir;

const WING: &str = "conversations";

// ---------------------------------------------------------------------------
// Copilot events.jsonl format
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CopilotEvent {
    #[serde(rename = "type")]
    event_type: String,
    data: Option<CopilotEventData>,
    timestamp: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CopilotEventData {
    content: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Claude JSONL format
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    #[serde(rename = "type")]
    msg_type: String,
    content: Option<serde_json::Value>,
    #[serde(rename = "userType")]
    user_type: Option<String>,
    timestamp: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn mine_copilot_sessions(
    palace_path: &str,
    embedder: Option<&Embedder>,
) -> Result<MineStats, MpError> {
    let session_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".copilot")
        .join("session-state");
    mine_sessions_dir(&session_dir, palace_path, "copilot", embedder)
}

pub fn mine_claude_sessions(
    palace_path: &str,
    embedder: Option<&Embedder>,
) -> Result<MineStats, MpError> {
    let projects_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("projects");
    mine_sessions_dir(&projects_dir, palace_path, "claude", embedder)
}

#[derive(Debug, Default)]
pub struct MineStats {
    pub sessions_processed: usize,
    pub sessions_skipped: usize,
    pub chunks_created: usize,
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Core logic
// ---------------------------------------------------------------------------

fn mine_sessions_dir(
    session_dir: &Path,
    palace_path: &str,
    source: &str,
    embedder: Option<&Embedder>,
) -> Result<MineStats, MpError> {
    if !session_dir.exists() {
        return Ok(MineStats::default());
    }

    let store = PalaceStore::open(palace_path)?;
    let mut stats = MineStats::default();

    // Collect all JSONL transcript files
    let jsonl_files: Vec<_> = WalkDir::new(session_dir)
        .follow_links(false)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().is_some_and(|x| x == "jsonl")
        })
        .collect();

    let total = jsonl_files.len();
    let dir_str = session_dir.to_string_lossy().to_string();
    let mut prog = MineProgress::new(&dir_str, WING, total);
    write_progress(&prog);

    for (idx, entry) in jsonl_files.iter().enumerate() {
        let path = entry.path();
        let source_file = path.to_string_lossy().to_string();

        // Skip files already mined (check by mtime)
        let mtime = std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64());

        if store.file_already_mined(&source_file, mtime)? {
            stats.sessions_skipped += 1;
            continue;
        }

        let text = match std::fs::read_to_string(path) {
            Ok(t) if !t.is_empty() => t,
            _ => { stats.sessions_skipped += 1; continue; }
        };

        let messages = if source == "copilot" {
            extract_copilot_messages(&text)
        } else {
            extract_claude_messages(&text)
        };

        if messages.is_empty() {
            stats.sessions_skipped += 1;
            continue;
        }

        let combined = messages.join("\n\n---\n\n");
        let session_name = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("session_{idx}"));
        let room = format!("{source}_{session_name}");

        store.delete_file_drawers(&source_file)?;

        let chunks = crate::mining::chunk_text_pub(&combined, 800, 100);
        let texts: Vec<&str> = chunks.iter().map(|s| s.as_str()).collect();
        let embeddings = embedder.and_then(|e| e.embed(&texts).ok());

        for (i, chunk) in chunks.iter().enumerate() {
            let emb = embeddings.as_ref().and_then(|e| e.get(i).map(|v| v.as_slice()));
            store.upsert_drawer(
                WING, &room, chunk, "conversation",
                Some(&source_file), mtime, Some(i as i64), emb,
            )?;
            stats.chunks_created += 1;
        }

        stats.sessions_processed += 1;
        prog.files_done = idx + 1;
        prog.chunks_created = stats.chunks_created;
        prog.current_file = session_name;
        write_progress(&prog);
    }

    prog.status = MineStatus::Done;
    prog.files_done = stats.sessions_processed;
    prog.end_unix = Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
    );
    write_progress(&prog);

    Ok(stats)
}

// ---------------------------------------------------------------------------
// Format extractors
// ---------------------------------------------------------------------------

fn extract_copilot_messages(jsonl: &str) -> Vec<String> {
    let mut messages = Vec::new();
    for line in jsonl.lines() {
        let Ok(event) = serde_json::from_str::<CopilotEvent>(line) else { continue };
        if matches!(event.event_type.as_str(), "user.message" | "assistant.message" | "agent.message") {
            if let Some(data) = event.data {
                if let Some(content) = data.content {
                    if !content.trim().is_empty() {
                        let role = if event.event_type == "user.message" { "User" } else { "Assistant" };
                        messages.push(format!("{role}: {content}"));
                    }
                }
            }
        }
    }
    messages
}

fn extract_claude_messages(jsonl: &str) -> Vec<String> {
    let mut messages = Vec::new();
    for line in jsonl.lines() {
        let Ok(msg) = serde_json::from_str::<ClaudeMessage>(line) else { continue };
        if msg.msg_type != "message" { continue; }
        let content = match &msg.content {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join(" "),
            _ => continue,
        };
        if content.trim().is_empty() { continue; }
        let role = msg.user_type.as_deref().unwrap_or("unknown");
        messages.push(format!("{role}: {content}"));
    }
    messages
}
