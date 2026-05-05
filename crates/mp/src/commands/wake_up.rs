use anyhow::Result;
use mempalace::{MempalaceConfig, storage::PalaceStore, kg::KnowledgeGraph};

pub fn run(palace_path: Option<&str>) -> Result<()> {
    let config = MempalaceConfig::load();
    let path = palace_path.unwrap_or(&config.palace_path);

    let store = PalaceStore::open(path)?;
    let stats = store.get_stats()?;
    println!("=== MemPalace Wake-Up ===");
    println!("Total drawers: {}", stats["total_drawers"]);

    let kg = KnowledgeGraph::open(&MempalaceConfig::knowledge_graph_path())?;
    let kg_stats = kg.stats()?;
    println!("KG entities: {} | triples: {}", kg_stats["entity_count"], kg_stats["triple_count"]);

    println!("\nRecent diary:");
    let recent = store.list_drawers(Some("wing_agent"), None, Some(5))?;
    for m in recent {
        let d = store.get_drawer(&m.drawer_id)?;
        let preview = if d.content.len() > 200 { &d.content[..200] } else { &d.content };
        println!("  [{}] {preview}", m.room);
    }
    Ok(())
}
