#!/usr/bin/env bash
# install.sh — Register MemPalace skill with GitHub Copilot CLI
#
# What this does:
#   1. Copies SKILL.md to the Copilot CLI skills directory
#   2. Configures the MCP server so Copilot CLI can call MemPalace tools
#   3. Appends MemPalace wake-up instructions to ~/.copilot/copilot-instructions.md
#   4. Writes a copilot() wrapper to .bashrc/.zshrc so sessions are auto-mined
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
# Preflight — locate or download the binary
# ---------------------------------------------------------------------------
if ! command -v "$MEMPALACE_BIN" &>/dev/null; then
    warn "mempalace not found — attempting to download pre-built release"
    if command -v curl &>/dev/null || command -v wget &>/dev/null; then
        GET_SCRIPT="$(mktemp)"
        if command -v curl &>/dev/null; then
            curl -fsSL "https://raw.githubusercontent.com/amosdavis/mempalace/main/get.sh" -o "$GET_SCRIPT"
        else
            wget -qO "$GET_SCRIPT" "https://raw.githubusercontent.com/amosdavis/mempalace/main/get.sh"
        fi
        bash "$GET_SCRIPT"
        rm -f "$GET_SCRIPT"
        for candidate in "$HOME/.local/bin/mempalace" "$HOME/.cargo/bin/mempalace" /usr/local/bin/mempalace; do
            [ -x "$candidate" ] && MEMPALACE_BIN="$candidate" && break
        done
    fi
    command -v "$MEMPALACE_BIN" &>/dev/null || \
        fail "mempalace not found. Download: curl -fsSL https://raw.githubusercontent.com/amosdavis/mempalace/main/get.sh | bash"
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
# copilot-instructions.md — inject auto-wake-up directive
# ---------------------------------------------------------------------------
# This is the ONLY mechanism that fires at the start of every Copilot session.
# Without it, mempalace_wake_up is never called and the palace sits idle.
INSTRUCTIONS_FILE="$COPILOT_HOME/copilot-instructions.md"
INSTRUCTIONS_MARKER="<!-- MEMPALACE_WAKE_UP_INSTALLED -->"

if grep -q "$INSTRUCTIONS_MARKER" "$INSTRUCTIONS_FILE" 2>/dev/null; then
    ok "MemPalace wake-up already in $INSTRUCTIONS_FILE"
else
    touch "$INSTRUCTIONS_FILE"
    cat >> "$INSTRUCTIONS_FILE" << 'INSTREOF'

# MemPalace Memory System
<!-- MEMPALACE_WAKE_UP_INSTALLED -->
You have a local memory palace (mempalace MCP server) connected via MCP tools.

**MANDATORY: At the very start of every new session, call `mempalace_wake_up` BEFORE responding to the first user message.** This loads palace statistics, recent diary entries, knowledge graph state, and the AAAK compression spec. Do not skip this step — without it you have no memory of prior conversations, people, or project context.

After wake-up, follow this protocol:
- **BEFORE** answering questions about any person, project, or past event: call `mempalace_search` or `mempalace_kg_query` first. Never guess — verify from the palace.
- **DURING** sessions: file important discoveries with `mempalace_add_drawer` immediately. Do not defer.
- **AFTER** each session: call `mempalace_diary_write` with `agent_name: "copilot"` and a summary of what happened and what you learned.

Use `/mempalace` to invoke the full skill for detailed tool documentation and slash commands.
INSTREOF
    ok "MemPalace wake-up directive written to $INSTRUCTIONS_FILE"
fi

# ---------------------------------------------------------------------------
WRAPPER_MARKER="# mempalace-copilot-wrapper"
WRAPPER=$(cat <<SHELLEOF

$WRAPPER_MARKER
# Auto-mines MemPalace after each Copilot CLI session.
# To disable: mempalace config set auto_mine_copilot_sessions false
function copilot() {
    command copilot "\$@"
    local _exit=\$?
    if "$ACTUAL_BIN" config get auto_mine_copilot_sessions 2>/dev/null | grep -q "^true$"; then
        "$ACTUAL_BIN" mine-sessions --copilot &>/dev/null &
        disown 2>/dev/null || true
    fi
    return \$_exit
}
SHELLEOF
)

add_wrapper_to_rc() {
    local rc="$1"
    if [ -f "$rc" ] && grep -q "$WRAPPER_MARKER" "$rc" 2>/dev/null; then
        ok "Shell wrapper already in $rc"
        return
    fi
    printf '%s\n' "$WRAPPER" >> "$rc"
    ok "Shell wrapper added to $rc"
}

# Detect shells in use
RC_FILES_UPDATED=0
for rc_candidate in "$HOME/.bashrc" "$HOME/.zshrc"; do
    if [ -f "$rc_candidate" ]; then
        add_wrapper_to_rc "$rc_candidate"
        RC_FILES_UPDATED=1
    fi
done

if [ "$RC_FILES_UPDATED" -eq 0 ]; then
    # Neither exists yet; default to .bashrc
    add_wrapper_to_rc "$HOME/.bashrc"
fi

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
echo "After each 'copilot' session, MemPalace mines it automatically."
echo "To disable: mempalace config set auto_mine_copilot_sessions false"
echo
echo "Backfill past Copilot sessions (run once):"
echo "  $ACTUAL_BIN mine-sessions --copilot"
echo
echo "Backfill past Claude sessions (run once):"
echo "  $ACTUAL_BIN mine-sessions --claude"
echo
echo "Reload your shell: source ~/.bashrc  (or ~/.zshrc)"

