#!/usr/bin/env bash
# install.sh — Register MemPalace skill with GitHub Copilot CLI
#
# What this does:
#   1. Copies SKILL.md to the Copilot CLI skills directory
#   2. Configures the MCP server so Copilot CLI can call MemPalace tools
#
# Usage:
#   ./install.sh
#   MEMPALACE_BIN=/path/to/mempalace ./install.sh
set -euo pipefail

MEMPALACE_BIN="${MEMPALACE_BIN:-mempalace}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Copilot CLI stores config in ~/.copilot by default; COPILOT_HOME overrides this.
COPILOT_HOME="${COPILOT_HOME:-$HOME/.copilot}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

ok()   { echo -e "${GREEN}✓${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC}  $*"; }
fail() { echo -e "${RED}✗${NC} $*"; exit 1; }

echo "MemPalace → GitHub Copilot CLI installer"
echo

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------
if ! command -v "$MEMPALACE_BIN" &>/dev/null; then
    fail "mempalace binary not found. Set MEMPALACE_BIN or add it to PATH."
fi
ACTUAL_BIN="$(command -v "$MEMPALACE_BIN")"
ok "Found mempalace: $ACTUAL_BIN"

# ---------------------------------------------------------------------------
# Install /mempalace skill
# ---------------------------------------------------------------------------
# Skills must live in their own subdirectory: ~/.copilot/skills/mempalace/SKILL.md
# Invoking: type "/mempalace ..." in any Copilot CLI prompt.
SKILL_DIR="$COPILOT_HOME/skills/mempalace"
mkdir -p "$SKILL_DIR"

cp "$SCRIPT_DIR/SKILL.md" "$SKILL_DIR/SKILL.md"
ok "Skill installed to $SKILL_DIR/SKILL.md  (invoke with /mempalace)"

# ---------------------------------------------------------------------------
# MCP server configuration
# ---------------------------------------------------------------------------
# Copilot CLI reads MCP servers from ~/.copilot/mcp-config.json
MCP_CONFIG="$COPILOT_HOME/mcp-config.json"

SERVER_ENTRY=$(cat <<JSON
{
  "mempalace": {
    "command": "$ACTUAL_BIN",
    "args": ["mcp"]
  }
}
JSON
)

merge_mcp_config() {
    local target="$1"
    local entry="$2"

    if [ ! -f "$target" ] || [ ! -s "$target" ]; then
        printf '{"mcpServers": %s}\n' "$entry" > "$target"
        return 0
    fi

    if command -v jq &>/dev/null; then
        local tmp
        tmp=$(mktemp)
        jq --argjson s "$entry" '.mcpServers = ($s + (.mcpServers // {}))' \
            "$target" > "$tmp" && mv "$tmp" "$target"
        return 0
    fi

    if command -v python3 &>/dev/null; then
        python3 - "$target" "$entry" <<'PYEOF'
import sys, json

path = sys.argv[1]
with open(path) as f:
    config = json.load(f)

new_servers = json.loads(sys.argv[2])
existing = config.get("mcpServers", {})
existing.update(new_servers)
config["mcpServers"] = existing

with open(path, "w") as f:
    json.dump(config, f, indent=2)
    f.write("\n")
PYEOF
        return 0
    fi

    warn "Neither jq nor python3 found — cannot merge MCP config automatically"
    warn "Add this to $target manually:"
    echo "$entry"
}

merge_mcp_config "$MCP_CONFIG" "$SERVER_ENTRY"
ok "MCP server configured in $MCP_CONFIG"

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ok "MemPalace is installed for GitHub Copilot CLI"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo
echo "Test it: type '/mempalace search for what we decided about auth'"
echo "         or ask 'What do you remember about me?' (Copilot auto-selects the skill)"
echo
echo "Backfill past sessions (optional):"
echo "  $ACTUAL_BIN mine ~/.claude/projects/ --wing conversations"
