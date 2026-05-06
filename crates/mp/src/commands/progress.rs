use anyhow::Result;
use mempalace::mining::progress::{list_active_jobs, read_progress};

pub fn run() -> Result<()> {
    let active = list_active_jobs();
    let legacy = read_progress();

    if active.is_empty() && legacy.is_none() {
        println!("—  No mining in progress (no state file found)");
        println!("   Run: mempalace mine <dir> --wing <wing>");
        return Ok(());
    }

    if !active.is_empty() {
        println!("Active mining jobs ({}):\n", active.len());
        for job in &active {
            let jid = job.job_id.as_deref().unwrap_or("unknown");
            println!("  Job: {jid}");
            println!("{}\n", job.format_display());
        }
    }

    if let Some(p) = legacy {
        if active.is_empty() {
            println!("{}", p.format_display());
        }
    }

    Ok(())
}
