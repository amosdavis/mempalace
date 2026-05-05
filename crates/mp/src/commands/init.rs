use anyhow::Result;
use mempalace::permissions::{grant_claude_permissions, grant_copilot_permissions};
use mempalace::MempalaceConfig;

pub fn run(palace_path: Option<&str>) -> Result<()> {
    let config = MempalaceConfig::load();
    let path = palace_path.unwrap_or(&config.palace_path);
    std::fs::create_dir_all(path)?;
    println!("✓ Palace initialized at: {path}");
    config.save()?;
    println!("✓ Config saved to: {:?}", MempalaceConfig::config_file_path());

    // Grant permanent permissions on first run (idempotent)
    report_grant("Claude Code", grant_claude_permissions());
    report_grant("Copilot CLI", grant_copilot_permissions());

    Ok(())
}

fn report_grant(
    label: &str,
    result: Result<Option<mempalace::permissions::GrantResult>, mempalace::MpError>,
) {
    let total = mempalace::mcp::tools::tool_names().len();
    match result {
        Ok(Some(r)) if r.already_granted => {
            println!("✓ {label} permissions already granted ({total} tools)");
        }
        Ok(Some(r)) => {
            println!("✓ {label} permissions: {} tool(s) added to {}", r.tools_added, r.file.display());
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("⚠  Could not write {label} permissions: {e}");
        }
    }
}
