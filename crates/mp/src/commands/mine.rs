use anyhow::Result;
use mempalace::{MempalaceConfig, storage::Embedder};
use std::path::Path;

pub fn run(project_dir: &str, wing: &str, force: bool, palace_path: Option<&str>) -> Result<()> {
    let config = MempalaceConfig::load();
    let palace_path = palace_path.unwrap_or(&config.palace_path);
    let embedder = Embedder::new(&config.embedding_device);

    println!("Mining: {project_dir} → wing:{wing} palace:{palace_path}");
    let stats = mempalace::mining::mine_project(
        Path::new(project_dir),
        palace_path,
        wing,
        Some(&embedder),
        force,
    )?;
    println!("  Files processed : {}", stats.files_processed);
    println!("  Files skipped   : {}", stats.files_skipped);
    println!("  Chunks created  : {}", stats.chunks_created);
    if !stats.errors.is_empty() {
        println!("  Errors          : {}", stats.errors.len());
    }
    Ok(())
}
