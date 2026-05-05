use anyhow::Result;
use mempalace::permissions::grant_claude_permissions;
use mempalace::MempalaceConfig;

pub fn run(palace_path: Option<&str>) -> Result<()> {
    let config = MempalaceConfig::load();
    let path = palace_path.unwrap_or(&config.palace_path);
    std::fs::create_dir_all(path)?;
    println!("✓ Palace initialized at: {path}");
    config.save()?;
    println!("✓ Config saved to: {:?}", MempalaceConfig::config_file_path());

    // Grant permanent permissions on first run (idempotent)
    match grant_claude_permissions() {
        Ok(Some(r)) if r.already_granted => {
            println!("✓ Claude permissions already granted ({} tools)", mempalace::mcp::tools::tool_names().len());
        }
        Ok(Some(r)) => {
            println!("✓ Granted Claude permissions: {} tools added to {}", r.tools_added, r.file.display());
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("⚠  Could not write Claude permissions ({}). Run `mempalace grant-permissions` later.", e);
        }
    }

    Ok(())
}
