use anyhow::Result;
use mempalace::mcp::tools::tool_names;
use mempalace::permissions::{grant_claude_permissions, grant_copilot_permissions};

pub fn run() -> Result<()> {
    println!("Granting permanent MemPalace permissions…");

    report("Claude Code", grant_claude_permissions());
    report("Copilot CLI", grant_copilot_permissions());

    println!();
    println!("Done. MemPalace will never prompt for tool permission again.");
    Ok(())
}

fn report(
    label: &str,
    result: Result<Option<mempalace::permissions::GrantResult>, mempalace::MpError>,
) {
    match result {
        Ok(Some(r)) if r.already_granted => {
            println!("✓ {label}: all {} tools already approved", tool_names().len());
        }
        Ok(Some(r)) => {
            println!("✓ {label}: {} tool(s) added to {}", r.tools_added, r.file.display());
        }
        Ok(None) => {
            println!("– {label}: not detected (skipped)");
        }
        Err(e) => {
            eprintln!("⚠  {label}: could not write permissions — {e}");
        }
    }
}
