#!/usr/bin/env bash
# install.sh — MemPalace integration installer
#
# Detects installed tools and configures MemPalace memory for each one.
# Run once; it handles everything. No manual steps required afterward.
#
# Usage:
#   ./install.sh                      # auto-detect and install for all
#   ./install.sh --claude-only        # Claude Code only
#   ./install.sh --copilot-only       # GitHub Copilot (CLI + VS Code) only
#   ./install.sh --vscode-only        # VS Code Copilot only
#   MEMPALACE_BIN=/path ./install.sh  # use specific binary
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEMPALACE_BIN="${MEMPALACE_BIN:-mempalace}"

# Parse flags
DO_CLAUDE=auto
DO_COPILOT_CLI=auto
DO_COPILOT_VSCODE=auto

for arg in "$@"; do
    case "$arg" in
        --claude-only)
            DO_CLAUDE=yes; DO_COPILOT_CLI=no; DO_COPILOT_VSCODE=no ;;
        --copilot-only)
            DO_CLAUDE=no; DO_COPILOT_CLI=yes; DO_COPILOT_VSCODE=yes ;;
        --vscode-only)
            DO_CLAUDE=no; DO_COPILOT_CLI=no; DO_COPILOT_VSCODE=yes ;;
        --help|-h)
            sed -n '2,12p' "$0" | sed 's/^# //'
            exit 0
            ;;
    esac
done

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'

ok()      { echo -e "  ${GREEN}✓${NC} $*"; }
warn()    { echo -e "  ${YELLOW}⚠${NC}  $*"; }
section() { echo -e "\n${BLUE}▶${NC} $*"; }
skip()    { echo -e "  ${YELLOW}–${NC} $* (not detected, skipping)"; }

echo
echo "  🏛  MemPalace Integration Installer"
echo "  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo

# ---------------------------------------------------------------------------
# Find the binary
# ---------------------------------------------------------------------------
section "Locating mempalace binary"

if ! command -v "$MEMPALACE_BIN" &>/dev/null; then
    # Try common install locations
    for candidate in \
        "$SCRIPT_DIR/target/release/mempalace" \
        "$SCRIPT_DIR/target/debug/mempalace" \
        "$HOME/.cargo/bin/mempalace"; do
        if [ -x "$candidate" ]; then
            MEMPALACE_BIN="$candidate"
            break
        fi
    done
fi

if ! command -v "$MEMPALACE_BIN" &>/dev/null && [ ! -x "$MEMPALACE_BIN" ]; then
    echo -e "  ${RED}✗${NC} mempalace binary not found."
    echo
    echo "  Build it first:"
    echo "    cargo install --path crates/mp"
    echo "  Or set MEMPALACE_BIN to the absolute path."
    exit 1
fi

ACTUAL_BIN="$(command -v "$MEMPALACE_BIN" 2>/dev/null || echo "$MEMPALACE_BIN")"
ok "Found: $ACTUAL_BIN"
export MEMPALACE_BIN="$ACTUAL_BIN"

# ---------------------------------------------------------------------------
# Initialize palace (idempotent)
# ---------------------------------------------------------------------------
section "Initializing palace"
"$ACTUAL_BIN" init 2>/dev/null && ok "Palace ready" || warn "Init returned non-zero (may already exist)"

# ---------------------------------------------------------------------------
# Claude Code
# ---------------------------------------------------------------------------
section "Claude Code"

should_install_claude() {
    [ "$DO_CLAUDE" = "yes" ] && return 0
    [ "$DO_CLAUDE" = "no" ] && return 1
    command -v claude &>/dev/null
}

if should_install_claude; then
    bash "$SCRIPT_DIR/integrations/claude-code/install.sh"
else
    skip "Claude Code (claude CLI not detected)"
fi

# ---------------------------------------------------------------------------
# GitHub Copilot CLI
# ---------------------------------------------------------------------------
section "GitHub Copilot CLI"

should_install_copilot_cli() {
    [ "$DO_COPILOT_CLI" = "yes" ] && return 0
    [ "$DO_COPILOT_CLI" = "no" ] && return 1
    command -v gh &>/dev/null
}

if should_install_copilot_cli; then
    bash "$SCRIPT_DIR/integrations/github-copilot-cli/install.sh"
else
    skip "GitHub Copilot CLI (gh not detected)"
fi

# ---------------------------------------------------------------------------
# VS Code Copilot
# ---------------------------------------------------------------------------
section "VS Code Copilot"

should_install_vscode() {
    [ "$DO_COPILOT_VSCODE" = "yes" ] && return 0
    [ "$DO_COPILOT_VSCODE" = "no" ] && return 1
    command -v code &>/dev/null || \
        [ -d "$HOME/.config/Code" ] || \
        [ -d "$HOME/Library/Application Support/Code" ]
}

if should_install_vscode; then
    bash "$SCRIPT_DIR/integrations/github-copilot-vscode/install.sh"
else
    skip "VS Code Copilot (VS Code not detected)"
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo
echo "  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "  ${GREEN}✓${NC}  MemPalace integration complete"
echo "  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo
echo "  Restart any open AI tools for changes to take effect."
echo
echo "  Backfill past Claude sessions (optional, run once):"
echo "    $ACTUAL_BIN mine ~/.claude/projects/ --wing conversations"
echo
