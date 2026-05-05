/// Grant all MemPalace MCP tools permanent permission in Claude Code and
/// any other AI tool whose permission model we know about.
///
/// Currently handles:
///   - Claude Code  → ~/.claude/settings.local.json  `permissions.allow[]`
///
/// This is idempotent: safe to call on every `mempalace init`.
use crate::error::MpError;
use crate::mcp::tools::tool_names;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub struct GrantResult {
    pub already_granted: bool,
    pub tools_added: usize,
    pub file: PathBuf,
}

/// Grant permanent permission for all MemPalace tools in Claude Code.
/// Returns `Ok(None)` if Claude settings are not present (Claude not installed).
pub fn grant_claude_permissions() -> Result<Option<GrantResult>, MpError> {
    let settings_path = claude_settings_path()?;

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
            // Create the nested structure
            let arr: Vec<serde_json::Value> = mcp_entries
                .iter()
                .map(|e| serde_json::Value::String(e.clone()))
                .collect();
            settings["permissions"] = serde_json::json!({ "allow": arr });
            (mcp_entries.len(), false)
        }
    };

    // Write back atomically
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = settings_path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&settings)?)?;
    std::fs::rename(&tmp, &settings_path)?;

    Ok(Some(GrantResult { already_granted, tools_added, file: settings_path }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn claude_settings_path() -> Result<PathBuf, MpError> {
    let home = dirs::home_dir().ok_or_else(|| {
        MpError::Validation("Cannot determine home directory".to_string())
    })?;
    Ok(home.join(".claude").join("settings.local.json"))
}
