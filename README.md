# MemPalace (Rust)

Local AI memory system. Stores your conversations verbatim, searches them semantically, and tracks facts over time — all on your machine, zero cloud, zero API keys.

## What It Does

- **Palace storage** — verbatim text in wings/rooms/drawers (SQLite, WAL mode)
- **Hybrid search** — BM25 + cosine similarity (0.6 vector / 0.4 BM25)
- **Knowledge graph** — temporal entity-relationship facts (who owns what, when it changed)
- **MCP server** — 30 tools for Claude Code, Copilot, Cursor, and any MCP host
- **Auto-hooks** — auto-mines transcripts in the background; zero chat interruption

## Install

### Pre-built binary (recommended, no Rust required)

```bash
curl -fsSL https://raw.githubusercontent.com/amosdavis/mempalace/main/get.sh | bash
```

Binaries are published for **Linux** (x86\_64, aarch64 — static musl), **macOS** (x86\_64, Apple Silicon), and **Windows** (x86\_64) on every tagged release.

Each binary is signed with [cosign](https://docs.sigstore.dev/) keyless signing via Sigstore. Verify manually:

```bash
cosign verify-blob \
  --bundle mempalace-<triple>.tar.gz.bundle \
  --certificate-identity-regexp "https://github.com/amosdavis/mempalace/.github/workflows/release.yml@refs/tags/" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  mempalace-<triple>.tar.gz
```

### From the Copilot CLI skill marketplace

```bash
gh skill install amosdavis/mempalace mempalace
```

### Build from source (requires Rust)

```bash
cargo install --path crates/mp
```

## Quick Start

```bash
# Initialize
mempalace init

# Mine a project
mempalace mine ~/my-project --wing project_myapp

# Search
mempalace search "what did we decide about authentication"

# Start MCP server (for AI tools)
mempalace mcp
```

## Integration — Install Once, Works Everywhere

```bash
./install.sh
```

Auto-detects installed AI tools, downloads the binary if needed, and configures MemPalace for each. Supports:
- **Claude Code** — MCP + Stop/PreCompact hooks + CLAUDE.md protocol
- **GitHub Copilot CLI** — `/mempalace` skill + MCP config (invoke with `/mempalace ...` in any prompt)
- **VS Code Copilot** — MCP server in settings.json + copilot-instructions.md

Flags:
```bash
./install.sh --claude-only     # Claude Code only
./install.sh --copilot-only    # Copilot CLI + VS Code only
./install.sh --vscode-only     # VS Code only
```

### Manual Integration

| Tool | Steps |
|------|-------|
| Claude Code | `cd integrations/claude-code && ./install.sh` |
| Copilot CLI  | `cd integrations/github-copilot-cli && ./install.sh` |
| VS Code      | `cd integrations/github-copilot-vscode && ./install.sh` |
| Any MCP host | `claude mcp add mempalace -- mempalace mcp` (adjust for your host) |

### After Install

Restart the AI tool. On the next session it will automatically:
1. Call `mempalace_status` to load the palace on wake-up
2. Search memories before responding about people or projects
3. Auto-mine the transcript every N messages in the background
4. Write a diary entry at session end (verbose mode) or silently (default)

**Copilot CLI**: use `/mempalace` as a slash-style invocation in any prompt:
```
/mempalace search for what we decided about authentication
/mempalace what do you know about the rust rewrite?
```
Copilot will also auto-select the skill when your prompt involves memory or recall.

## Commands

```
mempalace init               Initialize or verify the palace
mempalace mine <dir>         Mine a project directory
mempalace progress           Show mining status bar (elapsed, ETA, current file)
mempalace search <query>     Semantic search
mempalace status             Show palace stats
mempalace mcp                Start MCP JSON-RPC server (stdio)
mempalace mcp --install      Print Claude Code MCP add command
mempalace wake-up            Wake-up summary (top memories)
mempalace hook stop          Claude Code Stop hook handler
mempalace hook precompact    Claude Code PreCompact hook handler
```

## Configuration

Config file: `~/.mempalace/config.json` (created on first `init`)

| Env var | Default | Description |
|---------|---------|-------------|
| `MEMPALACE_PALACE_PATH` | `~/.mempalace/palace` | Palace directory |
| `MEMPALACE_EMBEDDING_DEVICE` | `cpu` | `cpu` or `cuda` |
| `MEMPALACE_SAVE_INTERVAL` | `15` | Hook trigger cadence (human messages) |
| `MEMPALACE_VERBOSE` | `false` | Block to prompt diary writes (hooks) |

## Architecture

```
User → CLI / MCP Server → SQLite (palace.sqlite3, WAL + FTS5)
                        → SQLite (knowledge_graph.sqlite3)

Palace:
  WING (person / project)
    └── ROOM (topic / day)
          └── DRAWER (verbatim text chunk)

Search:
  BM25 via SQLite FTS5 + cosine similarity on f32 BLOB embeddings
  Hybrid: 0.6 × vector + 0.4 × BM25

MCP: JSON-RPC 2.0 over stdio — 30 tools
```

## Design Principles

- **Verbatim always** — never summarize or paraphrase stored content
- **Local-first** — no external API required; embeddings run on-device
- **Append-only** — existing palace untouched on crash or partial mine
- **Background everything** — hooks run asynchronously; zero chat overhead

## Crate Structure

```
crates/
  mempalace/   library — storage, search, KG, mining, MCP tools
  mp/          binary  → installs as `mempalace`

integrations/
  claude-code/           Claude Code hooks + installer
  github-copilot-cli/    Copilot CLI skill + installer
  github-copilot-vscode/ VS Code settings + installer
```

## License

MIT
