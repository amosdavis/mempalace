use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Wire types — JSON shapes Claude Code sends to hooks
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
struct StopInput {
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    stop_hook_active: bool,
    #[serde(default)]
    transcript_path: String,
}

#[derive(Debug, Deserialize, Default)]
struct PrecompactInput {
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    transcript_path: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum HookOutput {
    Block { decision: &'static str, reason: String },
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn state_dir() -> PathBuf {
    let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(".mempalace").join("hook_state")
}

fn save_interval() -> u64 {
    std::env::var("MEMPALACE_SAVE_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(15)
}

fn is_verbose() -> bool {
    matches!(
        std::env::var("MEMPALACE_VERBOSE").as_deref(),
        Ok("true") | Ok("1") | Ok("yes")
    )
}

fn safe_session_id(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
        .take(128)
        .collect()
}

/// Count human (user) messages in a Claude Code JSONL transcript.
/// Returns 0 if the file cannot be read or parsed.
fn count_human_messages(transcript_path: &str) -> u64 {
    let path = Path::new(transcript_path);
    if !path.exists() {
        return 0;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return 0;
    };
    let mut count: u64 = 0;
    for line in text.lines() {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let role = val
            .get("message")
            .and_then(|m| m.get("role"))
            .and_then(|r| r.as_str())
            .unwrap_or("");
        if role != "user" {
            continue;
        }
        // Skip internal command-message turns (tool use results injected by the harness)
        let content = val
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("");
        if content.contains("<command-message>") {
            continue;
        }
        count += 1;
    }
    count
}

/// Spawn `mempalace mine <dir> --wing conversations` in background.
/// Silently drops errors — mining is best-effort in a hook context.
fn mine_background(dir: &Path) {
    let bin = std::env::current_exe()
        .ok()
        .unwrap_or_else(|| PathBuf::from("mempalace"));
    let _ = Command::new(bin)
        .arg("mine")
        .arg(dir)
        .arg("--wing")
        .arg("conversations")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn log(state_dir: &Path, msg: &str) {
    let log_path = state_dir.join("hook.log");
    use std::io::Write;
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(log_path) {
        let now = chrono::Local::now().format("%H:%M:%S");
        let _ = writeln!(f, "[{now}] {msg}");
    }
}

// ---------------------------------------------------------------------------
// Public entrypoints
// ---------------------------------------------------------------------------

/// `mempalace hook stop`
///
/// Reads Claude Code Stop-hook JSON from stdin, decides whether to block or
/// allow, and prints the appropriate JSON response to stdout.
pub fn run_stop() -> Result<()> {
    let mut raw = String::new();
    io::stdin().read_to_string(&mut raw)?;

    let input: StopInput = serde_json::from_str(&raw).unwrap_or_default();

    // Already in a save cycle — let the AI stop to prevent an infinite loop.
    if input.stop_hook_active {
        println!("{{}}");
        return Ok(());
    }

    let dir = state_dir();
    let _ = fs::create_dir_all(&dir);

    let sid = safe_session_id(&input.session_id);
    if sid.is_empty() {
        println!("{{}}");
        return Ok(());
    }

    let exchange_count = count_human_messages(&input.transcript_path);

    // Read last save point
    let last_save_file = dir.join(format!("{sid}_last_save"));
    let last_save: u64 = fs::read_to_string(&last_save_file)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let since_last = exchange_count.saturating_sub(last_save);
    let interval = save_interval();

    log(
        &dir,
        &format!(
            "Session {sid}: {exchange_count} exchanges, {since_last} since last save (interval={interval})"
        ),
    );

    if since_last >= interval && exchange_count > 0 {
        // Update last save point before triggering so concurrent hooks don't double-fire.
        let _ = fs::write(&last_save_file, exchange_count.to_string());

        log(&dir, &format!("TRIGGERING SAVE at exchange {exchange_count}"));

        // Mine the transcript directory in the background.
        let transcript_path = Path::new(&input.transcript_path);
        if transcript_path.extension().map_or(false, |e| e == "jsonl" || e == "json")
            && !input.transcript_path.contains("..")
            && transcript_path.exists()
        {
            if let Some(parent) = transcript_path.parent() {
                mine_background(parent);
            }
        }

        if is_verbose() {
            let output = HookOutput::Block {
                decision: "block",
                reason: "MemPalace save checkpoint. Write a brief session diary entry covering \
                    key topics, decisions, and code changes since the last save. Use verbatim \
                    quotes where possible. Call mempalace_diary_write to store it, then continue."
                    .to_string(),
            };
            println!("{}", serde_json::to_string(&output)?);
        } else {
            // Silent mode — auto-mine ran in background; don't interrupt the AI.
            println!("{{}}");
        }
    } else {
        println!("{{}}");
    }

    Ok(())
}

/// `mempalace hook precompact`
///
/// Fires right before the context window is compressed. Mines the transcript
/// synchronously, then optionally blocks to prompt a diary save.
pub fn run_precompact() -> Result<()> {
    let mut raw = String::new();
    io::stdin().read_to_string(&mut raw)?;

    let input: PrecompactInput = serde_json::from_str(&raw).unwrap_or_default();

    let dir = state_dir();
    let _ = fs::create_dir_all(&dir);

    let sid = safe_session_id(&input.session_id);
    log(&dir, &format!("PRE-COMPACT triggered for session {sid}"));

    let transcript_path = Path::new(&input.transcript_path);
    if transcript_path.extension().map_or(false, |e| e == "jsonl" || e == "json")
        && !input.transcript_path.contains("..")
        && transcript_path.exists()
    {
        if let Some(parent) = transcript_path.parent() {
            // Synchronous: mine must complete before compaction proceeds so
            // raw tool output is captured before the context window shrinks.
            let bin = std::env::current_exe()
                .ok()
                .unwrap_or_else(|| PathBuf::from("mempalace"));
            let status = Command::new(bin)
                .arg("mine")
                .arg(parent)
                .arg("--wing")
                .arg("conversations")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            log(
                &dir,
                &format!(
                    "Transcript mine: {}",
                    status.map_or_else(|e| e.to_string(), |s| s.to_string())
                ),
            );
        }
    }

    // Always silent for precompact: the compaction proceeds after the hook
    // returns. We captured the raw transcript; the AI will summarize on its own.
    println!("{{}}");
    Ok(())
}
