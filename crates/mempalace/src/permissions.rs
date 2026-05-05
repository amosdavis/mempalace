/// Grant all MemPalace MCP tools permanent permission in Claude Code and
/// GitHub Copilot CLI.
///
/// Handles:
///   - Claude Code  → ~/.claude/settings.local.json  `permissions.allow[]`
///   - Copilot CLI  → ~/.copilot/permissions-config.json  `locations[home]["tool_approvals"]`
///
/// Both operations are idempotent: safe to call on every `mempalace init`.
use crate::error::MpError;
use crate::mcp::tools::tool_names;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

pub struct GrantResult {
    pub already_granted: bool,
    pub tools_added: usize,
    pub file: PathBuf,
}

// ---------------------------------------------------------------------------
// Claude Code
// ---------------------------------------------------------------------------

/// Grant permanent permission for all MemPalace tools in Claude Code.
/// Returns `Ok(None)` if Claude settings directory does not exist.
pub fn grant_claude_permissions() -> Result<Option<GrantResult>, MpError> {
    let settings_path = home_path(&[".claude", "settings.local.json"])?;

    // Read existing settings (or start fresh)
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Build the permission entries: Claude MCP tool format is
    //   mcp__<server_name>__<tool_name>
    let mcp_entries: Vec<String> = tool_names()
        .iter()
        .map(|t| format!("mcp__mempalace__{t}"))
        .collect();

    let allow = settings
        .pointer_mut("/permissions/allow")
        .and_then(|v| v.as_array_mut());

    let (tools_added, already_granted) = match allow {
        Some(arr) => {
            let before = arr.len();
            for entry in &mcp_entries {
                if !arr.iter().any(|v| v.as_str() == Some(entry)) {
                    arr.push(serde_json::Value::String(entry.clone()));
                }
            }
            let added = arr.len() - before;
            (added, added == 0)
        }
        None => {
            let arr: Vec<serde_json::Value> = mcp_entries
                .iter()
                .map(|e| serde_json::Value::String(e.clone()))
                .collect();
            settings["permissions"] = serde_json::json!({ "allow": arr });
            (mcp_entries.len(), false)
        }
    };

    atomic_write_json(&settings_path, &settings)?;
    Ok(Some(GrantResult { already_granted, tools_added, file: settings_path }))
}

// ---------------------------------------------------------------------------
// GitHub Copilot CLI
// ---------------------------------------------------------------------------

/// Grant permanent permission for all MemPalace MCP tools in Copilot CLI.
/// Returns `Ok(None)` if `~/.copilot/` does not exist (Copilot not installed).
pub fn grant_copilot_permissions() -> Result<Option<GrantResult>, MpError> {
    let copilot_dir = home_path(&[".copilot"])?;
    if !copilot_dir.exists() {
        return Ok(None);
    }

    let perms_path = copilot_dir.join("permissions-config.json");
    let home = home_str()?;

    // Read or create the config
    let mut config: serde_json::Value = if perms_path.exists() {
        let content = std::fs::read_to_string(&perms_path)?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({ "locations": {} }))
    } else {
        serde_json::json!({ "locations": {} })
    };

    // Ensure locations[home][tool_approvals] exists
    if config["locations"].get(&home).is_none() {
        config["locations"][&home] = serde_json::json!({ "tool_approvals": [] });
    }
    if config["locations"][&home].get("tool_approvals").is_none() {
        config["locations"][&home]["tool_approvals"] = serde_json::json!([]);
    }

    let approvals = config["locations"][&home]["tool_approvals"]
        .as_array_mut()
        .ok_or_else(|| MpError::Validation("tool_approvals is not an array".to_string()))?;

    let tools = tool_names();
    let before = approvals.len();

    for tool in &tools {
        let already = approvals.iter().any(|entry| {
            entry.get("kind").and_then(|v| v.as_str()) == Some("mcp")
                && entry.get("serverName").and_then(|v| v.as_str()) == Some("mempalace")
                && entry.get("toolName").and_then(|v| v.as_str()) == Some(tool)
        });
        if !already {
            approvals.push(serde_json::json!({
                "kind": "mcp",
                "serverName": "mempalace",
                "toolName": tool
            }));
        }
    }

    let tools_added = approvals.len() - before;
    let already_granted = tools_added == 0;

    atomic_write_json(&perms_path, &config)?;
    Ok(Some(GrantResult { already_granted, tools_added, file: perms_path }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn home_dir() -> Result<PathBuf, MpError> {
    dirs::home_dir().ok_or_else(|| MpError::Validation("Cannot determine home directory".to_string()))
}

fn home_str() -> Result<String, MpError> {
    home_dir().map(|p| p.to_string_lossy().to_string())
}

fn home_path(components: &[&str]) -> Result<PathBuf, MpError> {
    let mut path = home_dir()?;
    for c in components { path = path.join(c); }
    Ok(path)
}

fn atomic_write_json(path: &PathBuf, value: &serde_json::Value) -> Result<(), MpError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
