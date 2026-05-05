# MemPalace — VS Code Copilot Integration

## Automatic Install

```bash
./install.sh
```

This adds the MemPalace MCP server to VS Code `settings.json` and installs the memory protocol instructions into `.github/copilot-instructions.md`.

## Manual Setup

### 1. Add MCP Server to VS Code Settings

Open VS Code settings (`Ctrl+,` → "Open Settings JSON") and add:

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

Replace `"mempalace"` with the absolute path to the binary if it's not on PATH.

### 2. Add Memory Protocol Instructions

Create or append to `.github/copilot-instructions.md` in your project:

```markdown
<!-- include integrations/github-copilot-vscode/copilot-instructions.md -->
```

Or copy the file directly:
```bash
mkdir -p .github
cp copilot-instructions.md .github/copilot-instructions.md
```

### 3. Reload VS Code

Run `Developer: Reload Window` from the command palette.

## Verification

In Copilot Chat, ask: *"What do you remember about me?"*

Copilot should:
1. Call `mempalace_status` (palace overview)
2. Call `mempalace_search` with a query about the user
3. Return results from the palace

If Copilot doesn't use MCP tools, check:
- VS Code version ≥ 1.99 (MCP support added here)
- `github.copilot.chat.mcp.enabled` is `true` in settings
- The `mempalace` binary is on PATH or the absolute path is correct

## Configuration

Set these environment variables before VS Code starts:

| Variable | Default | Description |
|----------|---------|-------------|
| `MEMPALACE_PALACE_PATH` | `~/.mempalace/palace` | Palace directory |
| `MEMPALACE_SAVE_INTERVAL` | `15` | Hook trigger cadence (messages) |
| `MEMPALACE_VERBOSE` | `false` | Block and prompt for diary writes |

## Backfill Past Sessions

Mine your past Claude Code sessions into the palace:

```bash
mempalace mine ~/.claude/projects/ --wing conversations
```
