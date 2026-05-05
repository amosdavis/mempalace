use anyhow::Result;
use mempalace::config::MempalaceConfig;
use mempalace::mining::sessions::{mine_claude_sessions, mine_copilot_sessions};

pub fn run_copilot(palace: Option<&str>) -> Result<()> {
    let cfg = MempalaceConfig::load();
    let palace_path = palace.unwrap_or(&cfg.palace_path);

    if !cfg.auto_mine_copilot_sessions {
        eprintln!("Copilot session auto-mining disabled (auto_mine_copilot_sessions=false)");
        return Ok(());
    }

    println!("Mining Copilot sessions…");
    let stats = mine_copilot_sessions(palace_path, None)?;
    println!(
        "Done. Sessions: {} processed, {} skipped. Chunks: {}.",
        stats.sessions_processed, stats.sessions_skipped, stats.chunks_created
    );
    Ok(())
}

pub fn run_claude(palace: Option<&str>) -> Result<()> {
    let cfg = MempalaceConfig::load();
    let palace_path = palace.unwrap_or(&cfg.palace_path);

    println!("Mining Claude sessions…");
    let stats = mine_claude_sessions(palace_path, None)?;
    println!(
        "Done. Sessions: {} processed, {} skipped. Chunks: {}.",
        stats.sessions_processed, stats.sessions_skipped, stats.chunks_created
    );
    Ok(())
}
