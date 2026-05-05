pub mod gitignore;
pub mod progress;

use crate::error::MpError;
use crate::storage::{PalaceStore, Embedder};
use gitignore::GitignoreFilter;
use progress::{MineProgress, MineStatus, write_progress};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use std::path::Path;

const CHUNK_SIZE: usize = 800;
const CHUNK_OVERLAP: usize = 100;
const EMBED_BATCH_SIZE: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MineStats {
    pub files_processed: usize,
    pub files_skipped: usize,
    pub chunks_created: usize,
    pub errors: Vec<String>,
}

static BINARY_EXTS: &[&str] = &[
    "exe","dll","so","dylib","a","lib","obj","o",
    "jpg","jpeg","png","gif","bmp","ico","webp",
    "mp3","mp4","wav","ogg","avi","mov","mkv",
    "zip","tar","gz","bz2","xz","7z","rar",
    "pdf","doc","docx","xls","xlsx","ppt","pptx",
    "bin","dat","db","sqlite","sqlite3",
    "pyc","pyo","class","lock","wasm",
];

static SKIP_DIRS: &[&str] = &[
    ".git",".svn",".hg",
    "node_modules","__pycache__",".pytest_cache",
    "target","dist","build",".build",
    ".venv","venv","env",
    ".mypy_cache",".ruff_cache",
    "coverage",".nyc_output",
];

pub fn mine_project(
    project_dir: &Path,
    palace_path: &str,
    wing: &str,
    embedder: Option<&Embedder>,
    force: bool,
) -> Result<MineStats, MpError> {
    let store = PalaceStore::open(palace_path)?;
    let gitignore = GitignoreFilter::new(project_dir);

    // --- Pre-scan: count mineable files so the progress bar has a denominator ---
    let files_total = count_mineable_files(project_dir, &gitignore);
    let dir_str = project_dir.to_string_lossy().to_string();
    let mut prog = MineProgress::new(&dir_str, wing, files_total);
    write_progress(&prog);

    let mut stats = MineStats {
        files_processed: 0,
        files_skipped: 0,
        chunks_created: 0,
        errors: Vec::new(),
    };

    let mut pending: Vec<(String, String, String, Option<f64>, i64)> = Vec::new();

    for entry in WalkDir::new(project_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            if e.file_type().is_dir() {
                return !SKIP_DIRS.contains(&name.as_ref());
            }
            true
        })
    {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => { stats.errors.push(e.to_string()); continue; }
        };
        if !entry.file_type().is_file() { continue; }
        let path = entry.path();

        if let Some(ext) = path.extension() {
            if BINARY_EXTS.contains(&ext.to_string_lossy().to_lowercase().as_ref()) {
                stats.files_skipped += 1;
                continue;
            }
        }
        if gitignore.is_ignored(path) { stats.files_skipped += 1; continue; }

        let mtime = std::fs::metadata(path).ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64());

        let source_file = path.to_string_lossy().to_string();

        if !force && store.file_already_mined(&source_file, mtime)? {
            stats.files_skipped += 1;
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) if !c.is_empty() => c,
            _ => { stats.files_skipped += 1; continue; }
        };

        let rel = path.strip_prefix(project_dir).unwrap_or(path)
            .to_string_lossy()
            .replace(['/', '\\', '.'], "-");
        let room = rel.trim_matches('-').to_string();
        let room = if room.is_empty() { "root".to_string() } else { room };

        store.delete_file_drawers(&source_file)?;

        for (i, chunk) in chunk_text(&content, CHUNK_SIZE, CHUNK_OVERLAP).into_iter().enumerate() {
            pending.push((chunk, room.clone(), source_file.clone(), mtime, i as i64));
        }
        stats.files_processed += 1;

        // Update progress after each file processed
        prog.files_done = stats.files_processed;
        prog.files_skipped = stats.files_skipped;
        prog.current_file = path.strip_prefix(project_dir).unwrap_or(path)
            .to_string_lossy()
            .to_string();
        write_progress(&prog);
    }

    for batch in pending.chunks(EMBED_BATCH_SIZE) {
        let texts: Vec<&str> = batch.iter().map(|(c, _, _, _, _)| c.as_str()).collect();
        let embeddings = embedder.and_then(|e| e.embed(&texts).ok());

        for (i, (content, room, source_file, mtime, chunk_idx)) in batch.iter().enumerate() {
            let emb = embeddings.as_ref().and_then(|e| e.get(i).map(|v| v.as_slice()));
            store.upsert_drawer(wing, room, content, "code",
                Some(source_file.as_str()), *mtime, Some(*chunk_idx), emb)?;
            stats.chunks_created += 1;
        }
        prog.chunks_created = stats.chunks_created;
        write_progress(&prog);
    }

    // Write final "done" state
    prog.status = MineStatus::Done;
    prog.files_done = stats.files_processed;
    prog.files_skipped = stats.files_skipped;
    prog.chunks_created = stats.chunks_created;
    prog.errors_count = stats.errors.len();
    prog.current_file = String::new();
    prog.end_unix = Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
    );
    write_progress(&prog);

    Ok(stats)
}

fn count_mineable_files(project_dir: &Path, gitignore: &GitignoreFilter) -> usize {
    WalkDir::new(project_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            if e.file_type().is_dir() {
                return !SKIP_DIRS.contains(&name.as_ref());
            }
            true
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let p = e.path();
            if let Some(ext) = p.extension() {
                if BINARY_EXTS.contains(&ext.to_string_lossy().to_lowercase().as_ref()) {
                    return false;
                }
            }
            !gitignore.is_ignored(p)
        })
        .count()
}

fn chunk_text(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    if total <= size { return vec![text.to_string()]; }
    let mut chunks = Vec::new();
    let step = size - overlap;
    let mut start = 0;
    while start < total {
        let end = (start + size).min(total);
        chunks.push(chars[start..end].iter().collect());
        if end == total { break; }
        start += step;
    }
    chunks
}
