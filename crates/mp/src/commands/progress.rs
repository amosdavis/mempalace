use anyhow::Result;
use mempalace::mining::progress::read_progress;

pub fn run() -> Result<()> {
    match read_progress() {
        None => {
            println!("—  No mining in progress (no state file found)");
            println!("   Run: mempalace mine <dir> --wing <wing>");
        }
        Some(p) => {
            println!("{}", p.format_display());
        }
    }
    Ok(())
}
