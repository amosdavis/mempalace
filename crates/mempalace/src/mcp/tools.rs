use crate::config::MempalaceConfig;
use crate::kg::KnowledgeGraph;
use crate::mining::progress::read_progress;
use crate::palace_graph::PalaceGraph;
use crate::search::hybrid::search_memories;
use crate::storage::{Embedder, PalaceStore};
use serde_json::{json, Value};

fn make_tool(name: &str, description: &str, schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": schema })
}

pub fn list_tools() -> Vec<Value> {
    vec![
        make_tool("mempalace_status", "Get palace status and overview",
            json!({"type":"object","properties":{"palace_path":{"type":"string"}},"required":[]})),
        make_tool("mempalace_search", "Search memories (hybrid BM25+vector)",
            json!({"type":"object","properties":{
                "query":{"type":"string"},
                "wing":{"type":"string"},
                "room":{"type":"string"},
                "n_results":{"type":"integer"},
                "max_distance":{"type":"number"},
                "vector_disabled":{"type":"boolean"}
            },"required":["query"]})),
        make_tool("mempalace_remember", "Save a memory",
            json!({"type":"object","properties":{
                "content":{"type":"string"},
                "wing":{"type":"string"},
                "room":{"type":"string"},
                "type":{"type":"string"}
            },"required":["content","wing","room"]})),
        make_tool("mempalace_diary_write", "Write diary entry",
            json!({"type":"object","properties":{
                "content":{"type":"string"},
                "wing":{"type":"string"}
            },"required":["content"]})),
        make_tool("mempalace_diary_read", "Read recent diary entries",
            json!({"type":"object","properties":{
                "wing":{"type":"string"},
                "n":{"type":"integer"}
            },"required":[]})),
        make_tool("mempalace_wing_list", "List all wings",
            json!({"type":"object","properties":{},"required":[]})),
        make_tool("mempalace_room_list", "List rooms in a wing",
            json!({"type":"object","properties":{"wing":{"type":"string"}},"required":["wing"]})),
        make_tool("mempalace_drawer_list", "List drawers",
            json!({"type":"object","properties":{
                "wing":{"type":"string"},
                "room":{"type":"string"},
                "limit":{"type":"integer"}
            },"required":[]})),
        make_tool("mempalace_drawer_read", "Read a drawer by ID",
            json!({"type":"object","properties":{"drawer_id":{"type":"string"}},"required":["drawer_id"]})),
        make_tool("mempalace_drawer_delete", "Delete a drawer",
            json!({"type":"object","properties":{"drawer_id":{"type":"string"}},"required":["drawer_id"]})),
        make_tool("mempalace_kg_add_entity", "Add KG entity",
            json!({"type":"object","properties":{
                "name":{"type":"string"},
                "entity_type":{"type":"string"},
                "attributes":{"type":"object"}
            },"required":["name"]})),
        make_tool("mempalace_kg_add_triple", "Add KG triple (subject→predicate→object)",
            json!({"type":"object","properties":{
                "subject":{"type":"string"},
                "predicate":{"type":"string"},
                "object":{"type":"string"},
                "valid_from":{"type":"string"},
                "valid_to":{"type":"string"},
                "source":{"type":"string"}
            },"required":["subject","predicate","object"]})),
        make_tool("mempalace_kg_query", "Query KG for an entity",
            json!({"type":"object","properties":{
                "name":{"type":"string"},
                "as_of":{"type":"string"},
                "direction":{"type":"string"}
            },"required":["name"]})),
        make_tool("mempalace_kg_invalidate", "Invalidate a KG triple",
            json!({"type":"object","properties":{
                "triple_id":{"type":"string"},
                "valid_to":{"type":"string"}
            },"required":["triple_id","valid_to"]})),
        make_tool("mempalace_kg_timeline", "Get KG timeline",
            json!({"type":"object","properties":{"limit":{"type":"integer"}},"required":[]})),
        make_tool("mempalace_kg_stats", "Get KG stats",
            json!({"type":"object","properties":{},"required":[]})),
        make_tool("mempalace_mine_project", "Mine a project directory (blocking; writes progress to ~/.mempalace/mine_progress.json)",
            json!({"type":"object","properties":{
                "project_dir":{"type":"string"},
                "wing":{"type":"string"},
                "force":{"type":"boolean"}
            },"required":["project_dir"]})),
        make_tool("mempalace_mine_status", "Show mining progress (status bar, elapsed, ETA, current file). Call while mining is running or after it finishes.",
            json!({"type":"object","properties":{},"required":[]})),
        make_tool("mempalace_tunnel_create", "Create a tunnel between rooms",
            json!({"type":"object","properties":{
                "from_wing":{"type":"string"},
                "from_room":{"type":"string"},
                "to_wing":{"type":"string"},
                "to_room":{"type":"string"},
                "note":{"type":"string"}
            },"required":["from_wing","from_room","to_wing","to_room"]})),
        make_tool("mempalace_tunnel_list", "List tunnels",
            json!({"type":"object","properties":{"wing":{"type":"string"}},"required":[]})),
        make_tool("mempalace_tunnel_delete", "Delete a tunnel",
            json!({"type":"object","properties":{"tunnel_id":{"type":"string"}},"required":["tunnel_id"]})),
        make_tool("mempalace_tunnel_follow", "Follow tunnels from a room",
            json!({"type":"object","properties":{
                "wing":{"type":"string"},
                "room":{"type":"string"}
            },"required":["wing","room"]})),
        make_tool("mempalace_graph_traverse", "Traverse palace graph from a room",
            json!({"type":"object","properties":{
                "wing":{"type":"string"},
                "room":{"type":"string"},
                "max_hops":{"type":"integer"}
            },"required":["wing","room"]})),
        make_tool("mempalace_graph_stats", "Get graph statistics",
            json!({"type":"object","properties":{},"required":[]})),
        make_tool("mempalace_graph_find_tunnels", "Find all tunnels in graph",
            json!({"type":"object","properties":{},"required":[]})),
        make_tool("mempalace_config_get", "Get current configuration",
            json!({"type":"object","properties":{},"required":[]})),
        make_tool("mempalace_palace_stats", "Get detailed palace statistics",
            json!({"type":"object","properties":{},"required":[]})),
        make_tool("mempalace_export_wing", "Export all drawers from a wing",
            json!({"type":"object","properties":{"wing":{"type":"string"}},"required":["wing"]})),
        make_tool("mempalace_bulk_remember", "Save multiple memories at once",
            json!({"type":"object","properties":{
                "entries":{"type":"array","items":{"type":"object"}},
                "wing":{"type":"string"}
            },"required":["entries","wing"]})),
        make_tool("mempalace_entity_list", "List KG entities",
            json!({"type":"object","properties":{"limit":{"type":"integer"}},"required":[]})),
        make_tool("mempalace_wake_up", "Wake-up protocol: status + recent diary + KG stats",
            json!({"type":"object","properties":{},"required":[]})),
    ]
}

/// Returns every tool name this MCP server exposes.
/// Used by the installer to pre-approve all tools in the host app's permission lists.
pub fn tool_names() -> Vec<&'static str> {
    vec![
        "mempalace_status",
        "mempalace_search",
        "mempalace_remember",
        "mempalace_diary_write",
        "mempalace_diary_read",
        "mempalace_wing_list",
        "mempalace_room_list",
        "mempalace_drawer_list",
        "mempalace_drawer_read",
        "mempalace_drawer_delete",
        "mempalace_kg_add_entity",
        "mempalace_kg_add_triple",
        "mempalace_kg_query",
        "mempalace_kg_invalidate",
        "mempalace_kg_timeline",
        "mempalace_kg_stats",
        "mempalace_mine_project",
        "mempalace_mine_status",
        "mempalace_tunnel_create",
        "mempalace_tunnel_list",
        "mempalace_tunnel_delete",
        "mempalace_tunnel_follow",
        "mempalace_graph_traverse",
        "mempalace_graph_stats",
        "mempalace_graph_find_tunnels",
        "mempalace_config_get",
        "mempalace_palace_stats",
        "mempalace_export_wing",
        "mempalace_bulk_remember",
        "mempalace_entity_list",
        "mempalace_wake_up",
    ]
}

pub fn call_tool(name: &str, args: &Value, store: &PalaceStore, _palace_path: &str) -> Result<Value, anyhow::Error> {
    let config = MempalaceConfig::load();

    match name {
        "mempalace_status" => {
            let stats = store.get_stats()?;
            Ok(json!({
                "palace_path": "redb",
                "stats": stats,
                "protocol": crate::PALACE_PROTOCOL,
            }))
        }

        "mempalace_search" => {
            let query = args["query"].as_str().unwrap_or("");
            let wing = args.get("wing").and_then(|v| v.as_str());
            let room = args.get("room").and_then(|v| v.as_str());
            let n = args.get("n_results").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
            let max_dist = args.get("max_distance").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32;
            let vec_off = args.get("vector_disabled").and_then(|v| v.as_bool()).unwrap_or(false);
            let embedder = Embedder::new(&config.embedding_device);
            let result = search_memories(query, store, Some(&embedder), wing, room, n, max_dist, vec_off)?;
            Ok(serde_json::to_value(&result)?)
        }

        "mempalace_remember" => {
            let content = args["content"].as_str().unwrap_or("");
            let wing = args["wing"].as_str().unwrap_or("wing_user");
            let room = args["room"].as_str().unwrap_or("general");
            let type_ = args.get("type").and_then(|v| v.as_str()).unwrap_or("text");
            let embedder = Embedder::new(&config.embedding_device);
            let embs = embedder.embed(&[content]).ok();
            let emb = embs.as_ref().and_then(|e| e.first().map(|v| v.as_slice()));
            let id = store.upsert_drawer(wing, room, content, type_, None, None, None, emb)?;
            Ok(json!({"drawer_id": id, "wing": wing, "room": room, "status": "saved"}))
        }

        "mempalace_diary_write" => {
            let content = args["content"].as_str().unwrap_or("");
            let wing = args.get("wing").and_then(|v| v.as_str()).unwrap_or("wing_agent");
            let room = chrono::Utc::now().format("diary-%Y-%m-%d").to_string();
            let embedder = Embedder::new(&config.embedding_device);
            let embs = embedder.embed(&[content]).ok();
            let emb = embs.as_ref().and_then(|e| e.first().map(|v| v.as_slice()));
            let id = store.upsert_drawer(wing, &room, content, "diary", None, None, None, emb)?;
            Ok(json!({"drawer_id": id, "wing": wing, "room": room, "status": "saved"}))
        }

        "mempalace_diary_read" => {
            let wing = args.get("wing").and_then(|v| v.as_str()).unwrap_or("wing_agent");
            let n = args.get("n").and_then(|v| v.as_i64()).unwrap_or(10);
            let drawers = store.list_drawers(Some(wing), None, Some(n))?;
            let entries: Vec<Value> = drawers.into_iter().map(|m| {
                let content = store.get_drawer(&m.drawer_id).map(|d| d.content).unwrap_or_default();
                json!({"drawer_id": m.drawer_id, "room": m.room, "content": content})
            }).collect();
            Ok(json!({"entries": entries, "wing": wing}))
        }

        "mempalace_wing_list" => {
            Ok(json!({"wings": store.list_wings()?}))
        }

        "mempalace_room_list" => {
            let wing = args["wing"].as_str().unwrap_or("");
            Ok(json!({"wing": wing, "rooms": store.list_rooms(wing)?}))
        }

        "mempalace_drawer_list" => {
            let wing = args.get("wing").and_then(|v| v.as_str());
            let room = args.get("room").and_then(|v| v.as_str());
            let limit = args.get("limit").and_then(|v| v.as_i64()).or(Some(20));
            let drawers = store.list_drawers(wing, room, limit)?;
            Ok(serde_json::to_value(&drawers)?)
        }

        "mempalace_drawer_read" => {
            let id = args["drawer_id"].as_str().unwrap_or("");
            let drawer = store.get_drawer(id)?;
            Ok(serde_json::to_value(&drawer)?)
        }

        "mempalace_drawer_delete" => {
            let id = args["drawer_id"].as_str().unwrap_or("");
            let deleted = store.delete_drawer(id)?;
            Ok(json!({"deleted": deleted, "drawer_id": id}))
        }

        "mempalace_kg_add_entity" => {
            let name = args["name"].as_str().unwrap_or("");
            let etype = args.get("entity_type").and_then(|v| v.as_str()).unwrap_or("person");
            let attrs = args.get("attributes").cloned().unwrap_or(json!({}));
            let kg = KnowledgeGraph::open(&MempalaceConfig::knowledge_graph_path())?;
            let id = kg.add_entity(name, etype, &attrs)?;
            Ok(json!({"entity_id": id, "name": name, "status": "saved"}))
        }

        "mempalace_kg_add_triple" => {
            let s = args["subject"].as_str().unwrap_or("");
            let p = args["predicate"].as_str().unwrap_or("");
            let o = args["object"].as_str().unwrap_or("");
            let vf = args.get("valid_from").and_then(|v| v.as_str());
            let vt = args.get("valid_to").and_then(|v| v.as_str());
            let src = args.get("source").and_then(|v| v.as_str());
            let kg = KnowledgeGraph::open(&MempalaceConfig::knowledge_graph_path())?;
            let id = kg.add_triple(s, p, o, vf, vt, src)?;
            Ok(json!({"triple_id": id, "status": "saved"}))
        }

        "mempalace_kg_query" => {
            let name = args["name"].as_str().unwrap_or("");
            let as_of = args.get("as_of").and_then(|v| v.as_str());
            let dir = args.get("direction").and_then(|v| v.as_str()).unwrap_or("both");
            let kg = KnowledgeGraph::open(&MempalaceConfig::knowledge_graph_path())?;
            let entity = kg.get_entity(name)?;
            let triples = kg.query_entity(name, as_of, dir)?;
            Ok(json!({"entity": entity, "triples": triples}))
        }

        "mempalace_kg_invalidate" => {
            let tid = args["triple_id"].as_str().unwrap_or("");
            let vt = args["valid_to"].as_str().unwrap_or("");
            let kg = KnowledgeGraph::open(&MempalaceConfig::knowledge_graph_path())?;
            let ok = kg.invalidate_triple(tid, vt)?;
            Ok(json!({"triple_id": tid, "invalidated": ok}))
        }

        "mempalace_kg_timeline" => {
            let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);
            let kg = KnowledgeGraph::open(&MempalaceConfig::knowledge_graph_path())?;
            Ok(json!({"triples": kg.timeline(limit)?}))
        }

        "mempalace_kg_stats" => {
            let kg = KnowledgeGraph::open(&MempalaceConfig::knowledge_graph_path())?;
            Ok(kg.stats()?)
        }

        "mempalace_mine_project" => {
            Err(anyhow::anyhow!(
                "mempalace_mine_project requires the async MCP server. \
                 Ensure mempalace is started via `mempalace mcp` (tokio runtime)."
            ))
        }

        "mempalace_mine_status" => {
            match read_progress() {
                None => Ok(json!({
                    "status": "idle",
                    "message": "No mining in progress. Run mempalace_mine_project to start."
                })),
                Some(p) => Ok(json!({
                    "status": format!("{:?}", p.status).to_lowercase(),
                    "display": p.format_display(),
                    "dir": p.dir,
                    "wing": p.wing,
                    "files_done": p.files_done,
                    "files_total": p.files_total,
                    "files_skipped": p.files_skipped,
                    "chunks_created": p.chunks_created,
                    "elapsed_secs": p.elapsed_secs(),
                    "eta_secs": p.eta_secs(),
                    "current_file": p.current_file,
                })),
            }
        }

        "mempalace_tunnel_create" => {
            let fw = args["from_wing"].as_str().unwrap_or("");
            let fr = args["from_room"].as_str().unwrap_or("");
            let tw = args["to_wing"].as_str().unwrap_or("");
            let tr = args["to_room"].as_str().unwrap_or("");
            let note = args.get("note").and_then(|v| v.as_str());
            let id = PalaceGraph::create_tunnel(store, fw, fr, tw, tr, note)?;
            Ok(json!({"tunnel_id": id, "status": "created"}))
        }

        "mempalace_tunnel_list" => {
            let wing = args.get("wing").and_then(|v| v.as_str());
            Ok(serde_json::to_value(store.list_tunnels(wing)?)?)
        }

        "mempalace_tunnel_delete" => {
            let id = args["tunnel_id"].as_str().unwrap_or("");
            let ok = PalaceGraph::delete_tunnel(store, id)?;
            Ok(json!({"tunnel_id": id, "deleted": ok}))
        }

        "mempalace_tunnel_follow" => {
            let wing = args["wing"].as_str().unwrap_or("");
            let room = args["room"].as_str().unwrap_or("");
            Ok(PalaceGraph::follow_tunnels(store, wing, room)?)
        }

        "mempalace_graph_traverse" => {
            let wing = args["wing"].as_str().unwrap_or("");
            let room = args["room"].as_str().unwrap_or("");
            let hops = args.get("max_hops").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
            Ok(PalaceGraph::traverse(store, wing, room, hops)?)
        }

        "mempalace_graph_stats" => {
            Ok(PalaceGraph::graph_stats(store)?)
        }

        "mempalace_graph_find_tunnels" => {
            Ok(PalaceGraph::find_tunnels(store)?)
        }

        "mempalace_config_get" => {
            Ok(serde_json::to_value(&config)?)
        }

        "mempalace_palace_stats" => {
            Ok(store.get_stats()?)
        }

        "mempalace_export_wing" => {
            let wing = args["wing"].as_str().unwrap_or("");
            let metas = store.list_drawers(Some(wing), None, Some(10000))?;
            let mut drawers: Vec<Value> = Vec::new();
            for m in &metas {
                if let Ok(d) = store.get_drawer(&m.drawer_id) {
                    drawers.push(serde_json::to_value(&d)?);
                }
            }
            let total = drawers.len();
            Ok(json!({"wing": wing, "drawers": drawers, "total": total}))
        }

        "mempalace_bulk_remember" => {
            let entries = args["entries"].as_array().cloned().unwrap_or_default();
            let wing = args["wing"].as_str().unwrap_or("wing_user");
            let embedder = Embedder::new(&config.embedding_device);
            let mut saved = 0usize;
            let total = entries.len();
            for entry in &entries {
                let content = entry.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let room = entry.get("room").and_then(|v| v.as_str()).unwrap_or("general");
                let type_ = entry.get("type").and_then(|v| v.as_str()).unwrap_or("text");
                if content.is_empty() { continue; }
                let embs = embedder.embed(&[content]).ok();
                let emb = embs.as_ref().and_then(|e| e.first().map(|v| v.as_slice()));
                store.upsert_drawer(wing, room, content, type_, None, None, None, emb)?;
                saved += 1;
            }
            Ok(json!({"saved": saved, "total": total}))
        }

        "mempalace_entity_list" => {
            let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);
            let kg = KnowledgeGraph::open(&MempalaceConfig::knowledge_graph_path())?;
            Ok(json!({"entities": kg.list_entities(limit)?}))
        }

        "mempalace_wake_up" => {
            let stats = store.get_stats()?;
            let kg = KnowledgeGraph::open(&MempalaceConfig::knowledge_graph_path())?;
            let kg_stats = kg.stats()?;
            let diary: Vec<Value> = store
                .list_drawers_with_content(Some("wing_agent"), None, 5)
                .unwrap_or_default()
                .into_iter()
                .map(|(m, content)| json!({"room": m.room, "content": content}))
                .collect();
            Ok(json!({
                "palace_stats": stats,
                "kg_stats": kg_stats,
                "recent_diary": diary,
                "protocol": crate::PALACE_PROTOCOL,
                "aaak_spec": crate::AAAK_SPEC,
            }))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {name}")),
    }
}
