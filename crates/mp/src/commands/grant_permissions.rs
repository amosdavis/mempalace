use anyhow::Result;
use mempalace::mcp::tools::tool_names;
use mempalace::permissions::grant_claude_permissions;

pub fn run() -> Result<()> {
    println!("Granting permanent MemPalace permissions…");

    match grant_claude_permissions()? {
        Some(r) if r.already_granted => {
            println!("✓ Claude Code: all {} tools already approved", tool_names().len());
        }
        Some(r) => {
            println!("✓ Claude Code: {} tool(s) added to {}", r.tools_added, r.file.display());
        }
        None => {
            println!("– Claude Code settings not found (not installed — skipping)");
        }
    }

    println!();
    println!("Done. MemPalace will never prompt for permission again in Claude Code.");
    Ok(())
}
