#!/usr/bin/env bash
# MemPalace Stop Hook — wraps `mempalace hook stop`
#
# Place this file anywhere. The mempalace binary must be on PATH,
# or set MEMPALACE_BIN to its absolute path.
#
# Configuration via environment variables:
#   MEMPALACE_BIN           — path to the mempalace binary (default: mempalace)
#   MEMPALACE_SAVE_INTERVAL — messages between saves (default: 15)
#   MEMPALACE_VERBOSE       — set to "true" to block and prompt diary writes
set -euo pipefail
MEMPALACE_BIN="${MEMPALACE_BIN:-mempalace}"
exec "$MEMPALACE_BIN" hook stop
