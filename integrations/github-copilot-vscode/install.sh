#!/usr/bin/env bash
# install.sh — Register MemPalace MCP server with GitHub Copilot in VS Code
#
# What this does:
#   1. Adds the mempalace MCP server entry to VS Code user settings.json
#   2. Optionally installs copilot-instructions.md into the repo's .github/ dir
#
# Usage:
#   ./install.sh                    # install MCP + prompt instructions
#   ./install.sh --no-instructions  # MCP only, skip instructions file
#   MEMPALACE_BIN=/path/to/mempalace ./install.sh
set -euo pipefail

MEMPALACE_BIN="${MEMPALACE_BIN:-mempalace}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_INSTRUCTIONS=true

for arg in "$@"; do
    case "$arg" in
        --no-instructions) INSTALL_INSTRUCTIONS=false ;;
    esac
done

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

ok()   { echo -e "${GREEN}✓${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC}  $*"; }
fail() { echo -e "${RED}✗${NC} $*"; exit 1; }

echo "MemPalace → VS Code Copilot installer"
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
# Locate VS Code settings.json
# ---------------------------------------------------------------------------
detect_vscode_settings() {
    # VS Code
    local candidates=(
        "$HOME/.config/Code/User/settings.json"                            # Linux
        "$HOME/Library/Application Support/Code/User/settings.json"        # macOS
        "$HOME/AppData/Roaming/Code/User/settings.json"                    # Windows native
        "/mnt/c/Users/$USER/AppData/Roaming/Code/User/settings.json"       # WSL
    )
    # VS Code Insiders
    local insiders=(
        "$HOME/.config/Code - Insiders/User/settings.json"
        "$HOME/Library/Application Support/Code - Insiders/User/settings.json"
    )
    for p in "${candidates[@]}" "${insiders[@]}"; do
        if [ -f "$p" ] || [ -d "$(dirname "$p")" ]; then
            echo "$p"
            return 0
        fi
    done
    echo ""
}

VSCODE_SETTINGS="$(detect_vscode_settings)"

if [ -z "$VSCODE_SETTINGS" ]; then
    warn "Could not detect VS Code settings.json location."
    warn "Please add manually (see README.md) or set VSCODE_SETTINGS env var."
    VSCODE_SETTINGS="${VSCODE_SETTINGS:-$HOME/.config/Code/User/settings.json}"
fi

mkdir -p "$(dirname "$VSCODE_SETTINGS")"

# ---------------------------------------------------------------------------
# Merge MCP server entry into VS Code settings.json
# ---------------------------------------------------------------------------
SERVER_ENTRY=$(cat <<JSON
{
  "mempalace": {
    "command": "$ACTUAL_BIN",
    "args": ["mcp"]
  }
}
JSON
)

merge_vscode_settings() {
    local target="$1"
    local entry="$2"

    if [ ! -f "$target" ] || [ ! -s "$target" ]; then
        cat > "$target" <<EOF
{
  "github.copilot.chat.mcp.servers": $entry
}
EOF
        return 0
    fi

    if command -v jq &>/dev/null; then
        local tmp
        tmp=$(mktemp)
        jq --argjson s "$entry" \
            '."github.copilot.chat.mcp.servers" = ($s + (."github.copilot.chat.mcp.servers" // {}))' \
            "$target" > "$tmp" && mv "$tmp" "$target"
        return 0
    fi

    if command -v python3 &>/dev/null; then
        python3 - "$target" "$entry" <<'PYEOF'
import sys, json

path = sys.argv[1]
with open(path) as f:
    settings = json.load(f)

new_servers = json.loads(sys.argv[2])
key = "github.copilot.chat.mcp.servers"
existing = settings.get(key, {})
existing.update(new_servers)
settings[key] = existing

with open(path, "w") as f:
    json.dump(settings, f, indent=2)
    f.write("\n")
PYEOF
        return 0
    fi

    warn "Neither jq nor python3 available — cannot auto-merge settings.json"
    warn "Add this to $target manually:"
    echo '"github.copilot.chat.mcp.servers": '"$entry"
}

merge_vscode_settings "$VSCODE_SETTINGS" "$SERVER_ENTRY"
ok "MCP server added to $VSCODE_SETTINGS"

# ---------------------------------------------------------------------------
# Install copilot-instructions.md
# ---------------------------------------------------------------------------
if [ "$INSTALL_INSTRUCTIONS" = "true" ]; then
    # Determine target: repo .github/ or user-level VS Code instructions
    REPO_GITHUB=".github"
    if git rev-parse --git-dir &>/dev/null 2>&1; then
        REPO_ROOT="$(git rev-parse --show-toplevel)"
        INSTRUCTIONS_TARGET="$REPO_ROOT/$REPO_GITHUB/copilot-instructions.md"
        mkdir -p "$REPO_ROOT/$REPO_GITHUB"
    else
        # Fall back to VS Code user instructions (if supported)
        INSTRUCTIONS_TARGET="$HOME/.config/Code/User/copilot-instructions.md"
    fi

    MARKER="<!-- mempalace-protocol -->"
    if [ -f "$INSTRUCTIONS_TARGET" ] && grep -q "$MARKER" "$INSTRUCTIONS_TARGET" 2>/dev/null; then
        warn "copilot-instructions.md already contains MemPalace protocol — skipping"
    else
        {
            if [ -f "$INSTRUCTIONS_TARGET" ]; then
                echo ""
            fi
            echo "$MARKER"
            cat "$SCRIPT_DIR/copilot-instructions.md"
        } >> "$INSTRUCTIONS_TARGET"
        ok "MemPalace protocol appended to $INSTRUCTIONS_TARGET"
    fi
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ok "MemPalace is installed for VS Code Copilot"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo
echo "Reload VS Code (or run 'Developer: Reload Window') for MCP to activate."
echo
echo "Test: open Copilot Chat and ask 'What do you remember about me?'"
echo "      Copilot should call mempalace_status then mempalace_search."
