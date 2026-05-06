use anyhow::Result;
use mempalace::MempalaceConfig;

pub fn run_server(palace_path: Option<&str>) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(mempalace::mcp::run_mcp_server_async(palace_path))?;
    Ok(())
}

pub fn print_install_cmd() {
    let config = MempalaceConfig::load();
    println!("To add MemPalace to Claude Code:");
    println!("  claude mcp add mempalace -- mempalace mcp");
    println!();
    println!("Palace path: {}", config.palace_path);
    println!("Config: {:?}", MempalaceConfig::config_file_path());
}
