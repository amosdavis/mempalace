use anyhow::Result;
use mempalace::MempalaceConfig;

pub fn run(palace_path: Option<&str>) -> Result<()> {
    let config = MempalaceConfig::load();
    let path = palace_path.unwrap_or(&config.palace_path);
    std::fs::create_dir_all(path)?;
    println!("✓ Palace initialized at: {path}");
    config.save()?;
    println!("✓ Config saved to: {:?}", MempalaceConfig::config_file_path());
    Ok(())
}
