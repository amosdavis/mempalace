#!/usr/bin/env bash
# install.sh — One-shot Claude Code integration for MemPalace (Rust)
#
# What this does:
#   1. Registers the mempalace MCP server with Claude Code
#   2. Installs Stop and PreCompact hooks into ~/.claude/settings.local.json
#   3. Appends the MemPalace memory protocol to ~/.claude/CLAUDE.md
#
# Usage:
#   ./install.sh                    # install using mempalace on PATH
#   MEMPALACE_BIN=/path/to/mempalace ./install.sh
#
# Requirements:
#   - mempalace binary on PATH (or MEMPALACE_BIN set)
#   - claude CLI on PATH (for MCP registration)
#   - python3 OR jq on PATH (for JSON merging)
set -euo pipefail

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
MEMPALACE_BIN="${MEMPALACE_BIN:-mempalace}"
CLAUDE_DIR="${CLAUDE_DIR:-$HOME/.claude}"
SETTINGS_FILE="$CLAUDE_DIR/settings.local.json"
CLAUDE_MD="$CLAUDE_DIR/CLAUDE.md"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK_DIR="$CLAUDE_DIR/hooks"

# Colors
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

ok()   { echo -e "${GREEN}✓${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC}  $*"; }
fail() { echo -e "${RED}✗${NC} $*"; exit 1; }

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------
echo "MemPalace → Claude Code installer"
echo

if ! command -v "$MEMPALACE_BIN" &>/dev/null; then
    fail "mempalace binary not found. Set MEMPALACE_BIN or add it to PATH."
fi
ok "Found mempalace: $(command -v "$MEMPALACE_BIN")"

# ---------------------------------------------------------------------------
# 1. Register MCP server
# ---------------------------------------------------------------------------
if command -v claude &>/dev/null; then
    if claude mcp add mempalace -- "$MEMPALACE_BIN" mcp 2>/dev/null; then
        ok "MCP server registered: mempalace"
    else
        warn "MCP already registered or claude mcp add failed — skipping"
    fi
else
    warn "claude CLI not found — skipping MCP registration"
    warn "Add manually: claude mcp add mempalace -- $MEMPALACE_BIN mcp"
fi

# ---------------------------------------------------------------------------
# 2. Install hook scripts
# ---------------------------------------------------------------------------
mkdir -p "$HOOK_DIR"

SAVE_HOOK="$HOOK_DIR/mempal_save_hook.sh"
PRECOMPACT_HOOK="$HOOK_DIR/mempal_precompact_hook.sh"

cp "$SCRIPT_DIR/mempal_save_hook.sh" "$SAVE_HOOK"
cp "$SCRIPT_DIR/mempal_precompact_hook.sh" "$PRECOMPACT_HOOK"
chmod +x "$SAVE_HOOK" "$PRECOMPACT_HOOK"
ok "Hook scripts installed to $HOOK_DIR"

# Replace MEMPALACE_BIN placeholder in installed hooks with the actual path
ACTUAL_BIN="$(command -v "$MEMPALACE_BIN")"
sed -i "s|MEMPALACE_BIN:-mempalace|MEMPALACE_BIN:-$ACTUAL_BIN|g" \
    "$SAVE_HOOK" "$PRECOMPACT_HOOK" 2>/dev/null || true

# ---------------------------------------------------------------------------
# 3. Merge hooks into settings.local.json
# ---------------------------------------------------------------------------
mkdir -p "$CLAUDE_DIR"

HOOKS_JSON=$(cat <<JSON
{
  "Stop": [{"matcher": "*", "hooks": [{"type": "command", "command": "$SAVE_HOOK", "timeout": 30}]}],
  "PreCompact": [{"hooks": [{"type": "command", "command": "$PRECOMPACT_HOOK", "timeout": 30}]}]
}
JSON
)

merge_json() {
    local target="$1"
    local hooks_fragment="$2"

    if [ ! -f "$target" ] || [ ! -s "$target" ]; then
        # Create a new settings file with just the hooks
        printf '{"hooks": %s}\n' "$hooks_fragment" > "$target"
        return 0
    fi

    # Try jq first (clean merge), then Python, then warn and append
    if command -v jq &>/dev/null; then
        local tmp
        tmp=$(mktemp)
        jq --argjson h "$hooks_fragment" '.hooks = ($h + (.hooks // {}))' \
            "$target" > "$tmp" && mv "$tmp" "$target"
        return 0
    fi

    if command -v python3 &>/dev/null; then
        python3 - "$target" "$hooks_fragment" <<'PYEOF'
import sys, json

path = sys.argv[1]
with open(path) as f:
    settings = json.load(f)

new_hooks = json.loads(sys.argv[2])
existing = settings.get("hooks", {})
existing.update(new_hooks)
settings["hooks"] = existing

with open(path, "w") as f:
    json.dump(settings, f, indent=2)
    f.write("\n")
PYEOF
        return 0
    fi

    warn "Neither jq nor python3 found — appending hooks section"
    warn "Please merge manually into $target"
    echo "Hooks JSON:"
    echo "$hooks_fragment"
}

merge_json "$SETTINGS_FILE" "$HOOKS_JSON"
ok "Hooks merged into $SETTINGS_FILE"

# ---------------------------------------------------------------------------
# 4. Append CLAUDE.md protocol
# ---------------------------------------------------------------------------
MARKER="<!-- mempalace-protocol -->"

if [ -f "$CLAUDE_MD" ] && grep -q "$MARKER" "$CLAUDE_MD" 2>/dev/null; then
    warn "CLAUDE.md already contains MemPalace protocol — skipping append"
else
    {
        echo ""
        echo "$MARKER"
        cat "$SCRIPT_DIR/CLAUDE.md"
        echo ""
    } >> "$CLAUDE_MD"
    ok "MemPalace protocol appended to $CLAUDE_MD"
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ok "MemPalace is installed for Claude Code"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo
echo "What happens now:"
echo "  • On every session Claude will call mempalace_status on wake-up"
echo "  • Every ${MEMPALACE_SAVE_INTERVAL:-15} messages the transcript is auto-mined"
echo "  • Before compaction the transcript is mined synchronously"
echo
echo "Optional: run a one-time backfill of past sessions:"
echo "  $MEMPALACE_BIN mine ~/.claude/projects/ --wing conversations"
echo
echo "Restart Claude Code for hooks to take effect."
