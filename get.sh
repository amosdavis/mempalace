#!/usr/bin/env bash
# get.sh — Download and install the mempalace binary from GitHub Releases
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/amosdavis/mempalace/main/get.sh | bash
#   ./get.sh                         # latest release
#   ./get.sh --version v3.3.3        # specific version
#   ./get.sh --dest /usr/local/bin   # custom install directory
#   ./get.sh --no-verify             # skip cosign signature verification
set -euo pipefail

REPO="${REPO:-amosdavis/mempalace}"
BINARY="mempalace"
DEST="${DEST:-}"
VERSION="${VERSION:-}"
VERIFY="${VERIFY:-true}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

ok()   { echo -e "  ${GREEN}✓${NC} $*"; }
warn() { echo -e "  ${YELLOW}⚠${NC}  $*"; }
fail() { echo -e "  ${RED}✗${NC} $*" >&2; exit 1; }
info() { echo -e "  ${BLUE}▶${NC} $*"; }

for arg in "$@"; do
    case "$arg" in
        --version=*) VERSION="${arg#*=}" ;;
        --dest=*)    DEST="${arg#*=}" ;;
        --no-verify) VERIFY=false ;;
        --version)   shift; VERSION="${1:-}" ;;
        --dest)      shift; DEST="${1:-}" ;;
    esac
done

echo
echo "  🏛  MemPalace Installer"
echo "  ━━━━━━━━━━━━━━━━━━━━━━"
echo

# ---------------------------------------------------------------------------
# Detect platform
# ---------------------------------------------------------------------------
info "Detecting platform"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64)  TRIPLE="x86_64-unknown-linux-musl" ;;
            aarch64) TRIPLE="aarch64-unknown-linux-musl" ;;
            arm64)   TRIPLE="aarch64-unknown-linux-musl" ;;
            *)       fail "Unsupported Linux architecture: $ARCH" ;;
        esac
        ARCHIVE_EXT="tar.gz"
        ;;
    Darwin)
        case "$ARCH" in
            x86_64) TRIPLE="x86_64-apple-darwin" ;;
            arm64)  TRIPLE="aarch64-apple-darwin" ;;
            *)      fail "Unsupported macOS architecture: $ARCH" ;;
        esac
        ARCHIVE_EXT="tar.gz"
        ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
        TRIPLE="x86_64-pc-windows-msvc"
        ARCHIVE_EXT="zip"
        BINARY="${BINARY}.exe"
        ;;
    *)
        fail "Unsupported operating system: $OS"
        ;;
esac

ok "Platform: $TRIPLE"

# ---------------------------------------------------------------------------
# Resolve version
# ---------------------------------------------------------------------------
info "Resolving version"

if [ -z "$VERSION" ]; then
    if command -v curl &>/dev/null; then
        VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
    elif command -v wget &>/dev/null; then
        VERSION="$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
    else
        fail "curl or wget is required"
    fi
fi

[ -z "$VERSION" ] && fail "Could not determine release version"
ok "Version: $VERSION"

# ---------------------------------------------------------------------------
# Determine install destination
# ---------------------------------------------------------------------------
if [ -z "$DEST" ]; then
    if [ -w /usr/local/bin ]; then
        DEST="/usr/local/bin"
    elif [ -d "$HOME/.local/bin" ]; then
        DEST="$HOME/.local/bin"
    elif [ -d "$HOME/.cargo/bin" ]; then
        DEST="$HOME/.cargo/bin"
    else
        DEST="$HOME/.local/bin"
        mkdir -p "$DEST"
    fi
fi
mkdir -p "$DEST"
ok "Install destination: $DEST"

# ---------------------------------------------------------------------------
# Download
# ---------------------------------------------------------------------------
info "Downloading $BINARY $VERSION"

BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
ASSET="${BINARY%-*}-${TRIPLE}.${ARCHIVE_EXT}"     # e.g. mempalace-x86_64-unknown-linux-musl.tar.gz
ASSET="${BINARY%.*}-${TRIPLE}.${ARCHIVE_EXT}"      # strip .exe suffix for asset name
ASSET="mempalace-${TRIPLE}.${ARCHIVE_EXT}"
BUNDLE="${ASSET}.bundle"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

download() {
    local url="$1" dest="$2"
    if command -v curl &>/dev/null; then
        curl -fsSL --progress-bar -o "$dest" "$url"
    else
        wget -q --show-progress -O "$dest" "$url"
    fi
}

download "${BASE_URL}/${ASSET}" "${TMPDIR}/${ASSET}"
ok "Downloaded $ASSET"

# ---------------------------------------------------------------------------
# Verify signature with cosign (optional but recommended)
# ---------------------------------------------------------------------------
if [ "$VERIFY" = "true" ]; then
    info "Verifying signature"

    if command -v cosign &>/dev/null; then
        download "${BASE_URL}/${BUNDLE}" "${TMPDIR}/${BUNDLE}" 2>/dev/null || {
            warn "Signature bundle not found — skipping verification (run with --no-verify to suppress)"
            VERIFY=skipped
        }

        if [ "$VERIFY" = "true" ]; then
            cosign verify-blob \
                --bundle "${TMPDIR}/${BUNDLE}" \
                --certificate-identity-regexp "https://github.com/${REPO}/.github/workflows/release.yml@refs/tags/" \
                --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
                "${TMPDIR}/${ASSET}"
            ok "Signature verified (Sigstore keyless)"
        fi
    else
        warn "cosign not found — skipping signature verification"
        warn "Install cosign: https://docs.sigstore.dev/cosign/system_config/installation/"
    fi
else
    warn "Signature verification skipped (--no-verify)"
fi

# ---------------------------------------------------------------------------
# Verify checksum
# ---------------------------------------------------------------------------
info "Verifying checksum"

SUMS_URL="${BASE_URL}/SHA256SUMS.txt"
SUMS_FILE="${TMPDIR}/SHA256SUMS.txt"

if download "$SUMS_URL" "$SUMS_FILE" 2>/dev/null; then
    cd "$TMPDIR"
    if command -v sha256sum &>/dev/null; then
        grep "$ASSET" "$SUMS_FILE" | sha256sum --check --status
        ok "SHA256 checksum verified"
    elif command -v shasum &>/dev/null; then
        grep "$ASSET" "$SUMS_FILE" | shasum -a 256 --check --status
        ok "SHA256 checksum verified"
    else
        warn "sha256sum/shasum not found — skipping checksum verification"
    fi
    cd - > /dev/null
else
    warn "Could not fetch SHA256SUMS.txt — skipping checksum verification"
fi

# ---------------------------------------------------------------------------
# Extract and install
# ---------------------------------------------------------------------------
info "Installing"

cd "$TMPDIR"
case "$ARCHIVE_EXT" in
    tar.gz)
        tar -xzf "$ASSET"
        ;;
    zip)
        if command -v unzip &>/dev/null; then
            unzip -q "$ASSET"
        elif command -v python3 &>/dev/null; then
            python3 -c "import zipfile,sys; zipfile.ZipFile('$ASSET').extractall()"
        else
            fail "unzip or python3 required to extract .zip on Windows"
        fi
        ;;
esac

install -m 755 "$BINARY" "$DEST/$BINARY"
cd - > /dev/null

ok "Installed $DEST/$BINARY"

# ---------------------------------------------------------------------------
# PATH check
# ---------------------------------------------------------------------------
if ! command -v "$BINARY" &>/dev/null; then
    echo
    warn "$DEST is not in your PATH. Add it:"
    warn "  export PATH=\"$DEST:\$PATH\""
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo
echo "  ━━━━━━━━━━━━━━━━━━━━━━"
ok  "mempalace $VERSION installed"
echo "  ━━━━━━━━━━━━━━━━━━━━━━"
echo
echo "  Next: run the integration installer"
echo "    curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | bash"
echo
echo "  Or initialize manually:"
echo "    $DEST/$BINARY init"
echo "    $DEST/$BINARY mcp   # start MCP server"
echo
