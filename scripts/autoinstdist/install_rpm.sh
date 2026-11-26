#!/usr/bin/env bash
# ==============================================================================
# Helper to install the latest built Velox RPM package from dist/
# ==============================================================================
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ ! -f "$REPO_DIR/VERSION" ]; then
    echo "Error: VERSION file not found at $REPO_DIR/VERSION" >&2
    exit 1
fi

VERSION="$(tr -d '[:space:]' < "$REPO_DIR/VERSION")"

RPM_PKG=$(ls -t "$REPO_DIR/dist/velox-${VERSION}"*.rpm 2>/dev/null | head -n 1 || true)
if [ -z "$RPM_PKG" ] || [ ! -f "$RPM_PKG" ]; then
    RPM_PKG=$(ls -t "$REPO_DIR/dist/"*.rpm 2>/dev/null | head -n 1 || true)
fi

if [ -z "$RPM_PKG" ] || [ ! -f "$RPM_PKG" ]; then
    echo "Error: No .rpm package found in $REPO_DIR/dist/ for version $VERSION" >&2
    echo "Run: bash scripts/packaging/fedora/build-rpm.sh" >&2
    exit 1
fi

echo "Installing $(basename "$RPM_PKG")..."
if command -v dnf >/dev/null 2>&1; then
    sudo dnf install -y "$RPM_PKG"
elif command -v rpm >/dev/null 2>&1; then
    sudo rpm -Uvh --replacepkgs "$RPM_PKG"
else
    echo "Error: Neither dnf nor rpm package manager found." >&2
    exit 1
fi

echo "Velox v$VERSION RPM installed successfully."
