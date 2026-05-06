pub mod gitignore;
pub mod progress;
pub mod sessions;

use crate::error::MpError;
use crate::storage::{PalaceStore, Embedder};
use gitignore::GitignoreFilter;
use progress::{MineProgress, MineStatus, write_progress};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use walkdir::WalkDir;

const CHUNK_SIZE: usize = 800;
const CHUNK_OVERLAP: usize = 100;

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
    mine_project_with_store(project_dir, &store, wing, embedder, force)
}

pub fn mine_project_with_store(
    project_dir: &Path,
    store: &PalaceStore,
    wing: &str,
    embedder: Option<&Embedder>,
    force: bool,
) -> Result<MineStats, MpError> {
    let gitignore = GitignoreFilter::new(project_dir);

    let files_total = count_mineable_files(project_dir, &gitignore);
    let dir_str = project_dir.to_string_lossy().to_string();
    let mut prog = MineProgress::new(&dir_str, wing, files_total);
    write_progress(&prog);

    let files: Vec<_> = WalkDir::new(project_dir)
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
            let path = e.path();
            if let Some(ext) = path.extension() {
                if BINARY_EXTS.contains(&ext.to_string_lossy().to_lowercase().as_ref()) {
                    return false;
                }
            }
            !gitignore.is_ignored(path)
        })
        .collect();

    let files_processed = Arc::new(AtomicUsize::new(0));
    let files_skipped = Arc::new(AtomicUsize::new(0));
    let chunks_created = Arc::new(AtomicUsize::new(0));
    let errors: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    struct FileChunks {
        chunks: Vec<String>,
        embeddings: Option<Vec<Vec<f32>>>,
        room: String,
        source_file: String,
        mtime: Option<f64>,
    }

    let (tx, rx) = std::sync::mpsc::sync_channel::<FileChunks>(64);

    std::thread::scope(|s| {
        let w_chunks_created = Arc::clone(&chunks_created);
        let w_errors = Arc::clone(&errors);
        let w_files_processed = Arc::clone(&files_processed);
        let w_files_skipped = Arc::clone(&files_skipped);

        let writer_handle = s.spawn(move || {
            let mut w_prog = MineProgress::new(&dir_str, wing, files_total);
            while let Ok(file_result) = rx.recv() {
                let chunks_with_emb: Vec<(&str, Option<&[f32]>)> = file_result
                    .chunks
                    .iter()
                    .enumerate()
                    .map(|(i, chunk)| {
                        let emb = file_result.embeddings.as_ref()
                            .and_then(|e| e.get(i).map(|v| v.as_slice()));
                        (chunk.as_str(), emb)
                    })
                    .collect();

                match store.replace_file_drawers(
                    wing,
                    &file_result.room,
                    &file_result.source_file,
                    &chunks_with_emb,
                    file_result.mtime,
                ) {
                    Ok(count) => {
                        w_chunks_created.fetch_add(count, Ordering::Relaxed);
                    }
                    Err(e) => {
                        if let Ok(mut errs) = w_errors.lock() {
                            errs.push(e.to_string());
                        }
                    }
                }

                w_prog.files_done = w_files_processed.load(Ordering::Relaxed);
                w_prog.files_skipped = w_files_skipped.load(Ordering::Relaxed);
                w_prog.chunks_created = w_chunks_created.load(Ordering::Relaxed);
                w_prog.current_file = file_result.source_file
                    .strip_prefix(&dir_str)
                    .unwrap_or(&file_result.source_file)
                    .trim_start_matches(['/', '\\'])
                    .to_string();
                write_progress(&w_prog);
            }
        });

        let project_dir_owned = project_dir.to_path_buf();
        files.par_iter().for_each(|entry| {
            let path = entry.path();
            let source_file = path.to_string_lossy().to_string();

            let mtime = std::fs::metadata(path).ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64());

            if !force {
                if let Ok(true) = store.file_already_mined(&source_file, mtime) {
                    files_skipped.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            }

            let content = match std::fs::read_to_string(path) {
                Ok(c) if !c.is_empty() => c,
                _ => {
                    files_skipped.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            };

            let rel = path.strip_prefix(&project_dir_owned).unwrap_or(path)
                .to_string_lossy()
                .replace(['/', '\\', '.'], "-");
            let room = rel.trim_matches('-').to_string();
            let room = if room.is_empty() { "root".to_string() } else { room };

            let chunks = chunk_text(&content, CHUNK_SIZE, CHUNK_OVERLAP);
            files_processed.fetch_add(1, Ordering::Relaxed);

            let embeddings = embedder.and_then(|e| {
                let texts: Vec<&str> = chunks.iter().map(|s| s.as_str()).collect();
                e.embed(&texts).ok()
            });

            let _ = tx.send(FileChunks {
                chunks,
                embeddings,
                room,
                source_file,
                mtime,
            });
        });

        drop(tx);
        writer_handle.join().unwrap();
    });

    let final_errors = Arc::try_unwrap(errors)
        .unwrap_or_else(|arc| arc.lock().unwrap().clone().into())
        .into_inner()
        .unwrap_or_default();

    let stats = MineStats {
        files_processed: files_processed.load(Ordering::Relaxed),
        files_skipped: files_skipped.load(Ordering::Relaxed),
        chunks_created: chunks_created.load(Ordering::Relaxed),
        errors: final_errors,
    };

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

pub fn chunk_text_pub(text: &str, size: usize, overlap: usize) -> Vec<String> {
    chunk_text(text, size, overlap)
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
