use anyhow::Result;
use mempalace::{MempalaceConfig, storage::PalaceStore};

pub fn run(palace_path: Option<&str>) -> Result<()> {
    let config = MempalaceConfig::load();
    let path = palace_path.unwrap_or(&config.palace_path);
    let store = PalaceStore::open(path)?;
    let stats = store.get_stats()?;
    println!("Palace: {path}");
    println!("Total drawers: {}", stats["total_drawers"]);
    let wings = store.list_wings()?;
    for wing in wings {
        let count = store.count_drawers(Some(&wing))?;
        let rooms = store.list_rooms(&wing)?;
        println!("  {wing}: {count} drawers in {} rooms", rooms.len());
    }
    Ok(())
}
