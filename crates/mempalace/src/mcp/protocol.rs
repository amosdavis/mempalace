use std::io::{BufRead, Write};
use serde_json::{Value, json};
use crate::config::MempalaceConfig;
use crate::mcp::tools;

pub fn run_mcp_server(palace_path: Option<&str>) -> Result<(), anyhow::Error> {
    let config = MempalaceConfig::load();
    let palace_path = palace_path
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.palace_path.clone());

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    eprintln!("[MemPalace MCP] Server started. Palace: {palace_path}");

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => { eprintln!("[MemPalace MCP] Read error: {e}"); break; }
        };
        if line.trim().is_empty() { continue; }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[MemPalace MCP] Parse error: {e}");
                let r = json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": {"code": -32700, "message": "Parse error", "data": e.to_string()}
                });
                let _ = writeln!(out, "{r}");
                let _ = out.flush();
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        eprintln!("[MemPalace MCP] method={method}");

        if method.starts_with("notifications/") { continue; }

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "mempalace", "version": "3.3.3"}
                }
            }),
            "ping" => json!({"jsonrpc": "2.0", "id": id, "result": {}}),
            "tools/list" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": {"tools": tools::list_tools()}
            }),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match tools::call_tool(name, &args, &palace_path) {
                    Ok(result) => json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {
                            "content": [{"type": "text", "text": result.to_string()}]
                        }
                    }),
                    Err(e) => json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": {"code": -32603, "message": "Internal error", "data": e.to_string()}
                    }),
                }
            }
            _ => json!({
                "jsonrpc": "2.0", "id": id,
                "error": {"code": -32601, "message": "Method not found",
                          "data": format!("Unknown method: {method}")}
            }),
        };

        let _ = writeln!(out, "{response}");
        let _ = out.flush();
    }
    Ok(())
}
