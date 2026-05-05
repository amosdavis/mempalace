---
name: mempalace
description: "MemPalace — Local AI memory with 100% recall target. Semantic search, temporal knowledge graph, palace architecture (wings/rooms/drawers). Free, no cloud, no API keys. Rust binary."
version: 3.3.3
homepage: https://github.com/MemPalace/mempalace
user-invocable: true
metadata:
  copilot:
    emoji: "🏛"
    os:
      - linux
      - darwin
      - win32
    requires:
      anyBins:
        - mempalace
    install:
      - id: mempalace-cargo
        kind: cargo
        label: "Build MemPalace (Rust, SQLite backend)"
        crate: mp
        bins:
          - mempalace
---

# MemPalace — Local AI Memory System

You have access to a local memory palace via MCP tools. The palace stores verbatim conversation history and a temporal knowledge graph — all on the user's machine, zero cloud, zero API calls.

## Architecture

- **Wings** = people or projects (e.g. `people_alice`, `project_myapp`)
- **Rooms** = specific topics within a wing (e.g. `chromadb-setup`, `riley-school-2025`)
- **Drawers** = individual verbatim content chunks
- **Knowledge Graph** = typed entity-relationship facts with time validity windows

## Slash Invocations

Use `/mempalace` in any Copilot CLI prompt to invoke this skill explicitly:

```
/mempalace search for what we decided about authentication
/mempalace what do you know about the rust rewrite?
/mempalace progress                   ← show mining status bar
/mempalace mine ~/my-project          ← start mining a project
/mempalace what happened last session?
```

## Protocol — FOLLOW THIS EVERY SESSION

1. **ON WAKE-UP**: Call `mempalace_status` to load palace overview and AAAK dialect spec. Do this before anything else.
2. **BEFORE RESPONDING** about any person, project, or past event: call `mempalace_search` or `mempalace_kg_query` FIRST. Never guess — verify from the palace. Wrong is worse than slow.
3. **IF UNSURE** about a fact (name, status, relationship, preference): say "let me check" and query. Return the verbatim drawer text.
4. **DURING WORK**: File important discoveries and decisions immediately with `mempalace_add_drawer`. Do not defer to session end.
5. **AFTER EACH SESSION**: Call `mempalace_diary_write` to record what happened, what you learned, what changed.
6. **WHEN FACTS CHANGE**: Call `mempalace_kg_invalidate` on the old fact, `mempalace_kg_add` for the new one.

## Available Tools

### Mining
- `mempalace_mine_project` — Mine a project directory into the palace (blocking; writes live progress to disk).
  - `project_dir` (required): path to scan
  - `wing`: wing to store in (default `wing_code`)
  - `force`: re-mine files even if unchanged (default false)
- `mempalace_mine_status` — Show current mining progress: status bar, elapsed, ETA, current file.
  Call at any time — before, during, or after a mine run.
  Returns a `display` field with the full rendered output, plus raw counters.

### Search & Browse
- `mempalace_search` — Semantic search across all memories.
  - `query` (required): natural language keywords or question
  - `wing`: filter by wing
  - `room`: filter by room  
  - `limit`: max results (default 5)
- `mempalace_check_duplicate` — Check before filing to avoid duplicates.
  - `content` (required): text to check
  - `threshold`: similarity threshold (default 0.9)
- `mempalace_status` — Palace overview: total drawers, wings, rooms, AAAK spec
- `mempalace_list_wings` — All wings with drawer counts
- `mempalace_list_rooms` — Rooms within a wing
- `mempalace_get_taxonomy` — Full wing/room/count tree
- `mempalace_get_aaak_spec` — AAAK compression dialect specification

### Knowledge Graph (Temporal Facts)
- `mempalace_kg_query` — Query entity relationships.
  - `entity` (required): e.g. `Max`, `MyProject`
  - `as_of`: date filter (YYYY-MM-DD)
  - `direction`: `outgoing`, `incoming`, or `both` (default)
- `mempalace_kg_add` — Add a fact: subject → predicate → object
  - `subject`, `predicate`, `object` (required)
  - `valid_from`: when this became true
- `mempalace_kg_invalidate` — Mark a fact as no longer true
  - `subject`, `predicate`, `object` (required)
  - `ended`: when it stopped being true (default: today)
- `mempalace_kg_timeline` — Chronological story of an entity
- `mempalace_kg_stats` — Graph overview: entities, triples, relationship types

### Palace Graph (Cross-Domain Connections)
- `mempalace_traverse` — Walk from a room, find connected ideas across wings
  - `start_room` (required)
  - `max_hops`: connection depth (default 2)
- `mempalace_find_tunnels` — Find rooms bridging two wings
  - `wing_a`, `wing_b` (required)
- `mempalace_create_tunnel` — Create a cross-wing connection
- `mempalace_list_tunnels` — All tunnels
- `mempalace_graph_stats` — Connectivity overview

### Write
- `mempalace_add_drawer` — Store verbatim content.
  - `wing`, `room`, `content` (required)
  - `source_file`: optional source reference
- `mempalace_update_drawer` — Update existing drawer. `drawer_id`, `content`
- `mempalace_delete_drawer` — Remove a drawer. `drawer_id`
- `mempalace_diary_write` — Write a session diary entry.
  - `agent_name` (required): your name/identifier
  - `entry` (required): what happened, learned, matters
  - `topic`: category tag (default `general`)
- `mempalace_diary_read` — Read recent diary entries.
  - `agent_name` (required)
  - `last_n`: number of entries (default 10)

## MCP Setup

### GitHub Copilot CLI
The MCP server is configured automatically by the install script. To set up manually:

```bash
# Add to ~/.config/gh/copilot/skills/ or wherever Copilot CLI loads skills
mempalace mcp  # starts the JSON-RPC 2.0 server on stdio
```

### VS Code Copilot
Add to VS Code `settings.json`:
```json
{
  "github.copilot.chat.mcp.servers": {
    "mempalace": {
      "command": "mempalace",
      "args": ["mcp"]
    }
  }
}
```

### Other MCP Hosts
```bash
# Claude Code
claude mcp add mempalace -- mempalace mcp

# Cursor — add to .cursor/mcp.json
# Windsurf — add to mcp_config.json
```

## Tips

- Search is semantic (meaning-based), not keyword-exact. "What did we decide about the database schema?" works better than "database schema decision".
- The knowledge graph stores typed relationships with time windows — use it for facts about people and projects that change over time.
- Diary entries accumulate across sessions. Read them at wake-up to restore context from previous conversations.
- Use `mempalace_check_duplicate` before storing to avoid duplicates (default threshold 0.9 is conservative; 0.85 catches near-duplicates).
- The AAAK dialect (from `mempalace_status`) is a compact notation for scanning thousands of entries without reading all content.

## Filing Convention

| Category | Wing pattern | Example room |
|----------|-------------|--------------|
| People | `people_<firstname>` | `health-2025` |
| Projects | `project_<name>` | `backend-architecture` |
| Topics | `topics_<subject>` | `rust-async-patterns` |
| Conversations | `conversations` | `2025-05-session` |

Content is stored verbatim — never paraphrase, never summarize.
