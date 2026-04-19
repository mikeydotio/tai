#!/bin/sh
# tai installer — downloads the latest pre-built binary from GitHub Releases,
# verifies its SHA-256, and installs it to $HOME/.local/bin.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/mikeydotio/tai/main/scripts/install.sh | sh
#
# Env:
#   TAI_VERSION      Install a specific tag (e.g. v0.2.0) instead of latest.
#   TAI_INSTALL_DIR  Override install dir (default: $HOME/.local/bin).

set -eu

REPO="mikeydotio/tai"
INSTALL_DIR="${TAI_INSTALL_DIR:-$HOME/.local/bin}"

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

info() {
    printf '%s\n' "$*" >&2
}

# ---------------------------------------------------------------------------
# HTTP helpers — prefer curl, fall back to wget.
# ---------------------------------------------------------------------------

if command -v curl >/dev/null 2>&1; then
    HAS_CURL=1
elif command -v wget >/dev/null 2>&1; then
    HAS_CURL=
else
    die "need curl or wget on PATH"
fi

http_get() {
    # $1=url → stdout
    if [ -n "$HAS_CURL" ]; then
        curl -fsSL "$1"
    else
        wget -qO- "$1"
    fi
}

http_download() {
    # $1=url $2=output-path
    if [ -n "$HAS_CURL" ]; then
        curl -fsSL -o "$2" "$1"
    else
        wget -qO "$2" "$1"
    fi
}

# ---------------------------------------------------------------------------
# Platform detection → Rust target triple.
# ---------------------------------------------------------------------------

detect_target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$os/$arch" in
        Linux/x86_64|Linux/amd64)    echo "x86_64-unknown-linux-gnu" ;;
        Linux/aarch64|Linux/arm64)   echo "aarch64-unknown-linux-gnu" ;;
        Darwin/x86_64)               echo "x86_64-apple-darwin" ;;
        Darwin/arm64|Darwin/aarch64) echo "aarch64-apple-darwin" ;;
        *)
            die "unsupported platform: $os/$arch
supported:
  Linux   x86_64, aarch64
  Darwin  x86_64, arm64"
            ;;
    esac
}

# ---------------------------------------------------------------------------
# SHA-256 helper — sha256sum (Linux) or shasum -a 256 (macOS).
# ---------------------------------------------------------------------------

sha256_of() {
    # $1=file → lowercase hex digest
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        die "need sha256sum or shasum on PATH"
    fi
}

# ---------------------------------------------------------------------------
# Resolve version.
# ---------------------------------------------------------------------------

resolve_version() {
    if [ -n "${TAI_VERSION:-}" ]; then
        case "$TAI_VERSION" in
            v*) echo "$TAI_VERSION" ;;
            *)  echo "v$TAI_VERSION" ;;
        esac
        return
    fi

    tag=$(
        http_get "https://api.github.com/repos/$REPO/releases/latest" \
            | grep -m1 '"tag_name"' \
            | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/'
    ) || die "failed to fetch latest release metadata"

    [ -n "$tag" ] || die "could not parse latest release tag"
    echo "$tag"
}

# ---------------------------------------------------------------------------
# Main.
# ---------------------------------------------------------------------------

TARGET=$(detect_target)
VERSION=$(resolve_version)

TMPDIR=$(mktemp -d 2>/dev/null || mktemp -d -t tai-install)
trap 'rm -rf "$TMPDIR"' EXIT INT TERM

BIN_NAME="tai-$TARGET"
BASE_URL="https://github.com/$REPO/releases/download/$VERSION"

info "installing tai $VERSION for $TARGET"

http_download "$BASE_URL/$BIN_NAME"        "$TMPDIR/$BIN_NAME" \
    || die "download failed: $BASE_URL/$BIN_NAME"
http_download "$BASE_URL/sha256sums.txt"   "$TMPDIR/sha256sums.txt" \
    || die "download failed: $BASE_URL/sha256sums.txt"

expected=$(awk -v f="$BIN_NAME" '$2 == f {print $1; exit}' "$TMPDIR/sha256sums.txt")
[ -n "$expected" ] || die "no checksum line for $BIN_NAME in sha256sums.txt"

actual=$(sha256_of "$TMPDIR/$BIN_NAME")
[ "$expected" = "$actual" ] || die "checksum mismatch for $BIN_NAME
  expected $expected
  got      $actual"

mkdir -p "$INSTALL_DIR" || die "failed to create $INSTALL_DIR"
chmod +x "$TMPDIR/$BIN_NAME"
mv "$TMPDIR/$BIN_NAME" "$INSTALL_DIR/tai" || die "failed to install to $INSTALL_DIR/tai"

info "tai $VERSION installed to $INSTALL_DIR/tai"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        info ""
        info "warning: $INSTALL_DIR is not on PATH"
        info "  add this to your shell rc:"
        info "    export PATH=\"$INSTALL_DIR:\$PATH\""
        ;;
esac

info ""
info "run 'tai --help' to get started."
