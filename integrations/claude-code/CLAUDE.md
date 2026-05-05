# MemPalace Memory Protocol

You have access to a persistent local memory system via MCP tools. Every fact, conversation, decision, and preference you store is kept verbatim on this machine — no cloud, no summarization, no API calls. Memory is the foundation of identity; use it.

## Architecture

- **Wings** — broad categories (people, projects, topics, conversations)
- **Rooms** — specific topics within a wing
- **Drawers** — verbatim content chunks
- **Knowledge Graph** — typed entity-relationship facts with time validity

## Protocol — Follow This Every Session

1. **ON WAKE-UP**: Call `mempalace_status` immediately. This loads the palace overview, total drawer count, wing list, and AAAK dialect spec into context. Without this you are flying blind.

2. **BEFORE RESPONDING** about any person, project, past event, or prior decision: call `mempalace_search` or `mempalace_kg_query` FIRST. Never guess or assume from training — verify from the palace. Being wrong is worse than being slow.

3. **IF UNSURE** about a fact (someone's name, a project status, a preference, a relationship): say "let me check" and query the palace. Return the verbatim text from the drawer.

4. **DURING WORK**: When you encounter important decisions, discoveries, or user preferences — file them immediately with `mempalace_add_drawer`. Do not wait until session end.

5. **AFTER EACH SESSION**: Call `mempalace_diary_write` to record what happened, what you learned, what changed, and what matters next. Use verbatim quotes from the conversation.

6. **WHEN FACTS CHANGE**: Call `mempalace_kg_invalidate` on the outdated fact, then `mempalace_kg_add` for the new one. The knowledge graph tracks what was true *when*.

## Search Strategy

- Start with `mempalace_search` — semantic search across all drawers
- Use `mempalace_kg_query` for structured facts about people or projects
- Filter by wing/room when you know the context
- Use `mempalace_traverse` to find cross-domain connections
- Check `mempalace_diary_read` to recall what happened in recent sessions

## Available Tools

### Reading
- `mempalace_status` — Palace overview + AAAK spec (call on wake-up)
- `mempalace_search` — Semantic search. query, wing?, room?, limit?
- `mempalace_get_drawer` — Get a specific drawer by ID
- `mempalace_list_drawers` — List drawers in a wing/room
- `mempalace_list_wings` — All wings with counts
- `mempalace_list_rooms` — Rooms in a wing
- `mempalace_get_taxonomy` — Full wing/room/count tree
- `mempalace_diary_read` — Recent diary entries. agent_name, last_n?

### Knowledge Graph
- `mempalace_kg_query` — entity relationships. entity, as_of?, direction?
- `mempalace_kg_add` — Add fact: subject, predicate, object, valid_from?, source_closet?
- `mempalace_kg_invalidate` — Mark fact as no longer true. subject, predicate, object, ended?
- `mempalace_kg_timeline` — Chronological story of an entity
- `mempalace_kg_stats` — Graph overview

### Writing
- `mempalace_add_drawer` — Store verbatim content. wing, room, content, source_file?
- `mempalace_update_drawer` — Update existing drawer. drawer_id, content
- `mempalace_delete_drawer` — Remove a drawer. drawer_id
- `mempalace_check_duplicate` — Check before filing. content, threshold?
- `mempalace_diary_write` — Session diary. agent_name, entry, topic?

### Palace Graph
- `mempalace_traverse` — Walk from a room, find connected ideas. start_room, max_hops?
- `mempalace_find_tunnels` — Find rooms bridging two wings. wing_a, wing_b
- `mempalace_create_tunnel` — Create a cross-wing connection
- `mempalace_list_tunnels` — List all tunnels
- `mempalace_graph_stats` — Graph connectivity overview

## Filing Convention

Wing names: `people_<name>`, `project_<name>`, `topics_<subject>`, `conversations`  
Room names: kebab-case topic slugs (`chromadb-setup`, `riley-school-2025`)  
Content: verbatim — never paraphrase, never summarize, never lossy-compress

## Cost

Zero extra tokens in the default configuration. Hooks auto-mine and save in the background. The protocol above is the only overhead — and it is worth every token.
