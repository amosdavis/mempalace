use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::config::MempalaceConfig;
use crate::mcp::jobs::{JobManager, MineJobStatus};
use crate::mcp::tools;
use crate::mining::mine_project;
use crate::mining::progress::{MineProgress, MineStatus};
use crate::storage::{Embedder, PalaceStore};

pub async fn run_mcp_server_async(palace_path: Option<&str>) -> Result<(), anyhow::Error> {
    let config = MempalaceConfig::load();
    let palace_path = palace_path
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.palace_path.clone());

    let store = Arc::new(PalaceStore::open(&palace_path)?);
    let job_manager = Arc::new(JobManager::new(2));

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut out = stdout;

    eprintln!("[MemPalace MCP] Async server started. Palace: {palace_path}");

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[MemPalace MCP] Parse error: {e}");
                let r = json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": {"code": -32700, "message": "Parse error", "data": e.to_string()}
                });
                let mut resp = serde_json::to_string(&r)?;
                resp.push('\n');
                out.write_all(resp.as_bytes()).await?;
                out.flush().await?;
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("");
        eprintln!("[MemPalace MCP] method={method}");

        if method.starts_with("notifications/") {
            continue;
        }

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

                handle_tool_call(
                    name,
                    &args,
                    &palace_path,
                    &config,
                    &store,
                    &job_manager,
                    &id,
                )
                .await
            }
            _ => json!({
                "jsonrpc": "2.0", "id": id,
                "error": {"code": -32601, "message": "Method not found",
                          "data": format!("Unknown method: {method}")}
            }),
        };

        let mut resp = serde_json::to_string(&response)?;
        resp.push('\n');
        out.write_all(resp.as_bytes()).await?;
        out.flush().await?;
    }
    Ok(())
}

async fn handle_tool_call(
    name: &str,
    args: &Value,
    palace_path: &str,
    config: &MempalaceConfig,
    store: &Arc<PalaceStore>,
    job_manager: &Arc<JobManager>,
    id: &Value,
) -> Value {
    match name {
        "mempalace_mine_project" => {
            let dir = args["project_dir"]
                .as_str()
                .unwrap_or(".")
                .to_string();
            let wing = args
                .get("wing")
                .and_then(|v| v.as_str())
                .unwrap_or("wing_code")
                .to_string();
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

            let job_id = job_manager.submit_job(dir.clone(), wing.clone()).await;

            if !job_manager.can_start().await {
                return json!({
                    "jsonrpc": "2.0", "id": id.clone(),
                    "result": {
                        "content": [{"type": "text", "text": json!({
                            "status": "queued",
                            "job_id": &job_id,
                            "message": "Job queued — max concurrent jobs reached. Call mempalace_mine_status for updates."
                        }).to_string()}]
                    }
                });
            }

            let jm = Arc::clone(job_manager);
            let pp = palace_path.to_string();
            let ed = config.embedding_device.clone();
            let jid = job_id.clone();

            tokio::task::spawn_blocking(move || {
                let rt = tokio::runtime::Handle::current();
                let embedder = Embedder::new(&ed);
                let prog = MineProgress::new(&dir, &wing, 0).with_job_id(&jid);
                rt.block_on(jm.mark_running(&jid, prog));

                match mine_project(
                    std::path::Path::new(&dir),
                    &pp,
                    &wing,
                    Some(&embedder),
                    force,
                ) {
                    Ok(stats) => {
                        let mut final_prog = MineProgress::new(&dir, &wing, stats.files_processed)
                            .with_job_id(&jid);
                        final_prog.status = MineStatus::Done;
                        final_prog.files_done = stats.files_processed;
                        final_prog.chunks_created = stats.chunks_created;
                        rt.block_on(jm.mark_done(&jid, final_prog));
                    }
                    Err(e) => {
                        rt.block_on(jm.mark_failed(&jid, e.to_string()));
                    }
                }
            });

            json!({
                "jsonrpc": "2.0", "id": id.clone(),
                "result": {
                    "content": [{"type": "text", "text": json!({
                        "status": "started",
                        "job_id": &job_id,
                        "message": "Mining started in background. Call mempalace_mine_status for progress."
                    }).to_string()}]
                }
            })
        }
        "mempalace_mine_status" => {
            let specific_id = args.get("job_id").and_then(|v| v.as_str());

            if let Some(jid) = specific_id {
                let job = job_manager.get_job(jid).await;
                let file_progress = crate::mining::progress::read_progress_for_job(jid);
                let result = match job {
                    Some(mut j) => {
                        if file_progress.is_some() {
                            j.progress = file_progress;
                        }
                        serde_json::to_value(&j).unwrap_or(json!({"error": "serialize failed"}))
                    }
                    None => json!({"error": "job not found", "job_id": jid}),
                };
                return json!({
                    "jsonrpc": "2.0", "id": id.clone(),
                    "result": {
                        "content": [{"type": "text", "text": result.to_string()}]
                    }
                });
            }

            let jobs = job_manager.list_all().await;
            let active: Vec<_> = jobs
                .iter()
                .filter(|j| matches!(j.status, MineJobStatus::Queued | MineJobStatus::Running))
                .collect();

            let result = if active.is_empty() {
                json!({
                    "status": "idle",
                    "message": "No mining in progress.",
                    "completed_jobs": jobs.len()
                })
            } else {
                json!({
                    "status": "running",
                    "active_jobs": active.len(),
                    "jobs": active
                })
            };

            json!({
                "jsonrpc": "2.0", "id": id.clone(),
                "result": {
                    "content": [{"type": "text", "text": result.to_string()}]
                }
            })
        }
        "mempalace_mine_cancel" => {
            let job_id = args
                .get("job_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cancelled = job_manager.cancel_job(job_id).await;
            let result = json!({
                "cancelled": cancelled,
                "job_id": job_id
            });
            json!({
                "jsonrpc": "2.0", "id": id.clone(),
                "result": {
                    "content": [{"type": "text", "text": result.to_string()}]
                }
            })
        }
        _ => {
            match tools::call_tool(name, args, store, palace_path) {
                Ok(result) => json!({
                    "jsonrpc": "2.0", "id": id.clone(),
                    "result": {
                        "content": [{"type": "text", "text": result.to_string()}]
                    }
                }),
                Err(e) => json!({
                    "jsonrpc": "2.0", "id": id.clone(),
                    "error": {"code": -32603, "message": format!("{name} failed: {e}"), "data": e.to_string()}
                }),
            }
        }
    }
}

pub fn run_mcp_server(palace_path: Option<&str>) -> Result<(), anyhow::Error> {
    let config = MempalaceConfig::load();
    let palace_path = palace_path
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.palace_path.clone());

    let store = PalaceStore::open(&palace_path)?;

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    eprintln!("[MemPalace MCP] Server started. Palace: {palace_path}");

    use std::io::{BufRead, Write};
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[MemPalace MCP] Read error: {e}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }

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
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("");
        eprintln!("[MemPalace MCP] method={method}");

        if method.starts_with("notifications/") {
            continue;
        }

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
                match tools::call_tool(name, &args, &store, &palace_path) {
                    Ok(result) => json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {
                            "content": [{"type": "text", "text": result.to_string()}]
                        }
                    }),
                    Err(e) => json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": {"code": -32603, "message": format!("{name} failed: {e}"), "data": e.to_string()}
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

