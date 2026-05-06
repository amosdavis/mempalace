use anyhow::Result;
use mempalace::{MempalaceConfig, storage::Embedder};
use std::path::Path;

pub fn run(
    project_dir: &str,
    wing: &str,
    force: bool,
    background: bool,
    jobs: Option<usize>,
    palace_path: Option<&str>,
) -> Result<()> {
    let config = MempalaceConfig::load();
    let palace_path = palace_path.unwrap_or(&config.palace_path);

    if let Some(j) = jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(j)
            .build_global()
            .ok();
    }

    if background {
        let dir = project_dir.to_string();
        let wing_clone = wing.to_string();
        let pp = palace_path.to_string();
        let ed = config.embedding_device.clone();

        std::thread::spawn(move || {
            let embedder = Embedder::new(&ed);
            let _ = mempalace::mining::mine_project(
                Path::new(&dir),
                &pp,
                &wing_clone,
                Some(&embedder),
                force,
            );
        });
        println!("Mining started in background: {project_dir} → wing:{wing}");
        println!("Run `mempalace progress` to check status.");
        return Ok(());
    }

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
