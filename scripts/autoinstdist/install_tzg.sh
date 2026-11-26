#!/usr/bin/env bash
# ==============================================================================
# Helper to extract and install the latest built Velox tarball (.tar.gz) from dist/
# ==============================================================================
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ ! -f "$REPO_DIR/VERSION" ]; then
    echo "Error: VERSION file not found at $REPO_DIR/VERSION" >&2
    exit 1
fi

VERSION="$(tr -d '[:space:]' < "$REPO_DIR/VERSION")"

TAR_PKG=$(ls -t "$REPO_DIR/dist/velox-v${VERSION}"*.tar.gz 2>/dev/null | head -n 1 || true)
if [ -z "$TAR_PKG" ] || [ ! -f "$TAR_PKG" ]; then
    TAR_PKG=$(ls -t "$REPO_DIR/dist/"*.tar.gz 2>/dev/null | head -n 1 || true)
fi

if [ -z "$TAR_PKG" ] || [ ! -f "$TAR_PKG" ]; then
    echo "Error: No .tar.gz package found in $REPO_DIR/dist/ for version $VERSION" >&2
    echo "Run: bash scripts/packaging/tarball/build-tarball.sh" >&2
    exit 1
fi

TMP_DIR="$(mktemp -d "/tmp/velox-tar-install-XXXXXX")"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "Extracting $(basename "$TAR_PKG")..."
tar -xzf "$TAR_PKG" -C "$TMP_DIR"

BUNDLE_DIR="$(find "$TMP_DIR" -maxdepth 1 -mindepth 1 -type d | head -n 1)"
if [ -z "$BUNDLE_DIR" ] || [ ! -f "$BUNDLE_DIR/install.sh" ]; then
    echo "Error: install.sh not found inside tarball." >&2
    exit 1
fi

echo "Running installer from $BUNDLE_DIR..."
sudo bash "$BUNDLE_DIR/install.sh"

echo "Velox v$VERSION tarball installed successfully."
