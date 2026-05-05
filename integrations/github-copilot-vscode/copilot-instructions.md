# Use MemPalace for memory and context

You have access to a local memory palace via MCP tools (`mempalace_*`). The palace stores verbatim conversation history and a temporal knowledge graph — on the user's machine, no cloud, no API keys.

## Protocol

1. **ON WAKE-UP**: Call `mempalace_status` to load palace overview before doing anything else.
2. **BEFORE RESPONDING** about a person, project, or prior decision: call `mempalace_search` first. Do not guess — query the palace.
3. **AFTER COMPLETING WORK**: Call `mempalace_diary_write` to record what happened and what you learned.
4. **WHEN FACTS CHANGE**: Call `mempalace_kg_invalidate` on the outdated fact, then `mempalace_kg_add` for the new one.

## Key Tools

- `mempalace_status` — palace overview (call at session start)
- `mempalace_search` — semantic search; params: `query`, `wing?`, `room?`, `limit?`
- `mempalace_add_drawer` — store verbatim content; params: `wing`, `room`, `content`
- `mempalace_kg_query` — query entity relationships; params: `entity`, `as_of?`, `direction?`
- `mempalace_kg_add` — add fact; params: `subject`, `predicate`, `object`, `valid_from?`
- `mempalace_kg_invalidate` — mark fact outdated; params: `subject`, `predicate`, `object`
- `mempalace_diary_write` — session diary; params: `agent_name`, `entry`, `topic?`
- `mempalace_diary_read` — read past diary entries; params: `agent_name`, `last_n?`
- `mempalace_traverse` — find cross-domain connections; params: `start_room`, `max_hops?`

## Filing Convention

Wing names: `people_<name>`, `project_<name>`, `topics_<subject>`, `conversations`

Content is always stored verbatim — never paraphrase or summarize.
