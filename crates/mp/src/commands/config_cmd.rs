use anyhow::{bail, Result};
use mempalace::config::MempalaceConfig;

pub fn run_set(key: &str, value: &str) -> Result<()> {
    let mut cfg = MempalaceConfig::load();

    match key {
        "auto_mine_copilot_sessions" => {
            cfg.auto_mine_copilot_sessions = parse_bool(key, value)?;
        }
        "auto_mine_on_git_commit" => {
            cfg.auto_mine_on_git_commit = parse_bool(key, value)?;
        }
        "hook_silent_save" => {
            cfg.hook_silent_save = parse_bool(key, value)?;
        }
        "hook_desktop_toast" => {
            cfg.hook_desktop_toast = parse_bool(key, value)?;
        }
        "embedding_device" => {
            cfg.embedding_device = value.to_string();
        }
        "collection_name" => {
            cfg.collection_name = value.to_string();
        }
        _ => bail!("Unknown config key '{}'. Valid keys: auto_mine_copilot_sessions, auto_mine_on_git_commit, hook_silent_save, hook_desktop_toast, embedding_device, collection_name", key),
    }

    cfg.save()?;
    println!("Config updated: {key} = {value}");
    Ok(())
}

pub fn run_get(key: &str) -> Result<()> {
    let cfg = MempalaceConfig::load();
    let val = match key {
        "auto_mine_copilot_sessions" => cfg.auto_mine_copilot_sessions.to_string(),
        "auto_mine_on_git_commit" => cfg.auto_mine_on_git_commit.to_string(),
        "hook_silent_save" => cfg.hook_silent_save.to_string(),
        "hook_desktop_toast" => cfg.hook_desktop_toast.to_string(),
        "embedding_device" => cfg.embedding_device.clone(),
        "collection_name" => cfg.collection_name.clone(),
        "palace_path" => cfg.palace_path.clone(),
        _ => bail!("Unknown config key '{}'", key),
    };
    println!("{val}");
    Ok(())
}

fn parse_bool(key: &str, value: &str) -> anyhow::Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("Invalid boolean for '{}': expected true/false, got '{}'", key, value),
    }
}
