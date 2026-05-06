use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::config::MempalaceConfig;

macro_rules! debug_log {
    ($($arg:tt)*) => {
        if std::env::var("MEMPALACE_DEBUG").is_ok() {
            eprintln!($($arg)*);
        }
    };
}
use crate::mcp::jobs::{JobManager, MineJobStatus};
use crate::mcp::tools;
use crate::mining::mine_project_with_store;
use crate::mining::progress::{MineProgress, MineStatus};
use crate::storage::{Embedder, PalaceStore};

pub async fn run_mcp_server_async(palace_path: Option<&str>) -> Result<(), anyhow::Error> {
    let config = MempalaceConfig::load();
    let palace_path = palace_path
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.palace_path.clone());

    let max_jobs = config.max_concurrent_jobs.min(64).max(1);
    let store = Arc::new(PalaceStore::open(&palace_path)?);
    let job_manager = Arc::new(JobManager::new(max_jobs));

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut out = stdout;

    debug_log!("[MemPalace MCP] Async server started. Palace: {palace_path}");

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
                debug_log!("[MemPalace MCP] Parse error: {e}");
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
        debug_log!("[MemPalace MCP] method={method}");

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

            if let Some(c) = args.get("concurrency").and_then(|v| v.as_u64()) {
                job_manager.set_max_concurrent(c as usize);
            }

            let job_id = job_manager.submit_job(dir.clone(), wing.clone(), force).await;

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

            spawn_mine_job(
                Arc::clone(job_manager),
                Arc::clone(store),
                config.embedding_device.clone(),
                job_id.clone(),
                dir,
                wing,
                force,
            );

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
                let file_jobs = crate::mining::progress::list_active_jobs();
                let file_progress = crate::mining::progress::read_progress();

                if !file_jobs.is_empty() {
                    let running_count = file_jobs.iter()
                        .filter(|p| matches!(p.status, MineStatus::Running))
                        .count();
                    let total_files: usize = file_jobs.iter().map(|p| p.files_total).sum();
                    let done_files: usize = file_jobs.iter().map(|p| p.files_done).sum();
                    json!({
                        "status": "running",
                        "source": "filesystem",
                        "active_jobs": file_jobs.len(),
                        "running": running_count,
                        "total_files": total_files,
                        "files_done": done_files,
                        "message": format!("{} jobs active, {}/{} files processed. Use job_id param for details.", file_jobs.len(), done_files, total_files)
                    })
                } else if let Some(p) = file_progress {
                    if matches!(p.status, MineStatus::Running) {
                        json!({
                            "status": "running",
                            "source": "progress_file",
                            "dir": p.dir,
                            "wing": p.wing,
                            "files_done": p.files_done,
                            "files_total": p.files_total,
                            "chunks_created": p.chunks_created,
                            "elapsed_secs": p.elapsed_secs(),
                            "eta_secs": p.eta_secs()
                        })
                    } else {
                        json!({
                            "status": "idle",
                            "message": "No mining in progress."
                        })
                    }
                } else {
                    json!({
                        "status": "idle",
                        "message": "No mining in progress.",
                        "completed_jobs": jobs.len()
                    })
                }
            } else {
                let running: Vec<_> = active.iter()
                    .filter(|j| j.status == MineJobStatus::Running)
                    .collect();
                let queued: Vec<_> = active.iter()
                    .filter(|j| j.status == MineJobStatus::Queued)
                    .collect();
                let done_count = jobs.iter()
                    .filter(|j| j.status == MineJobStatus::Done)
                    .count();

                let short_dir = |d: &str| -> String {
                    std::path::Path::new(d)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| d.to_string())
                };

                let sample: Vec<_> = running.iter().take(5).map(|j| json!({
                    "job_id": j.job_id,
                    "dir": short_dir(&j.dir),
                    "status": "running"
                })).collect();

                json!({
                    "status": "running",
                    "running": running.len(),
                    "queued": queued.len(),
                    "done": done_count,
                    "total_jobs": jobs.len(),
                    "sample_running": sample,
                    "message": format!("{} running, {} queued, {} done. Pass job_id for details.", running.len(), queued.len(), done_count)
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

    debug_log!("[MemPalace MCP] Server started. Palace: {palace_path}");

    use std::io::{BufRead, Write};
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                debug_log!("[MemPalace MCP] Read error: {e}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                debug_log!("[MemPalace MCP] Parse error: {e}");
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
        debug_log!("[MemPalace MCP] method={method}");

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

fn spawn_mine_job(
    jm: Arc<JobManager>,
    store: Arc<PalaceStore>,
    embedding_device: String,
    job_id: String,
    dir: String,
    wing: String,
    force: bool,
) {
    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        let embedder = Embedder::new(&embedding_device);
        let prog = MineProgress::new(&dir, &wing, 0).with_job_id(&job_id);
        rt.block_on(jm.mark_running(&job_id, prog));

        match mine_project_with_store(
            std::path::Path::new(&dir),
            &store,
            &wing,
            Some(&embedder),
            force,
        ) {
            Ok(stats) => {
                let mut final_prog = MineProgress::new(&dir, &wing, stats.files_processed)
                    .with_job_id(&job_id);
                final_prog.status = MineStatus::Done;
                final_prog.files_done = stats.files_processed;
                final_prog.chunks_created = stats.chunks_created;
                rt.block_on(jm.mark_done(&job_id, final_prog));
            }
            Err(e) => {
                rt.block_on(jm.mark_failed(&job_id, e.to_string()));
            }
        }

        drain_queue(rt, jm, store, embedding_device);
    });
}

fn drain_queue(
    rt: tokio::runtime::Handle,
    jm: Arc<JobManager>,
    store: Arc<PalaceStore>,
    embedding_device: String,
) {
    while rt.block_on(jm.can_start()) {
        let next = rt.block_on(jm.next_queued());
        match next {
            Some(job) if !rt.block_on(jm.is_cancelled(&job.job_id)) => {
                let jm2 = Arc::clone(&jm);
                let store2 = Arc::clone(&store);
                let ed2 = embedding_device.clone();
                let jid = job.job_id.clone();
                let dir = job.dir.clone();
                let wing = job.wing.clone();
                let force = job.force;

                spawn_mine_job(jm2, store2, ed2, jid, dir, wing, force);
                break;
            }
            _ => break,
        }
    }
}
