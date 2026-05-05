use anyhow::Result;
use mempalace::{MempalaceConfig, storage::{Embedder, PalaceStore}, search::hybrid::search_memories};

pub fn run(
    query: &str,
    wing: Option<&str>,
    room: Option<&str>,
    n: usize,
    palace_path: Option<&str>,
) -> Result<()> {
    let config = MempalaceConfig::load();
    let palace_path = palace_path.unwrap_or(&config.palace_path);
    let store = PalaceStore::open(palace_path)?;
    let embedder = Embedder::new(&config.embedding_device);

    let result = search_memories(query, &store, Some(&embedder), wing, room, n, 1.5, false)?;
    println!("Results for: \"{query}\" ({} searched, vector={})",
        result.total_searched, result.vector_enabled);

    for (i, r) in result.results.iter().enumerate() {
        println!("\n[{}] {}/{} (dist={:.3}, score={:.3})",
            i + 1, r.wing, r.room, r.distance, r.score);
        let preview = if r.content.len() > 300 { &r.content[..300] } else { &r.content };
        println!("{preview}");
    }
    Ok(())
}
