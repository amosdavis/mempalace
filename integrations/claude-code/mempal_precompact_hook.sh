#!/usr/bin/env bash
# MemPalace PreCompact Hook — wraps `mempalace hook precompact`
#
# Fires right before Claude Code compresses the context window.
# Mines the active transcript synchronously, ensuring raw tool output
# is captured before it disappears.
#
# Configuration via environment variables:
#   MEMPALACE_BIN — path to the mempalace binary (default: mempalace)
set -euo pipefail
MEMPALACE_BIN="${MEMPALACE_BIN:-mempalace}"
exec "$MEMPALACE_BIN" hook precompact
