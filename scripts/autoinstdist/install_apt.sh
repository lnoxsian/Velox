#!/usr/bin/env bash
set -e

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ ! -f "$REPO_DIR/VERSION" ]; then
    echo "Error: VERSION file not found at $REPO_DIR/VERSION" >&2
    exit 1
fi

VERSION="$(tr -d '[:space:]' < "$REPO_DIR/VERSION")"

# Locate the corresponding .deb package in dist/
DEB_PKG=$(ls -t "$REPO_DIR/dist/velox_${VERSION}_"*.deb 2>/dev/null | head -n 1)

if [ -z "$DEB_PKG" ] || [ ! -f "$DEB_PKG" ]; then
    DEB_PKG=$(ls -t "$REPO_DIR/dist/"*.deb 2>/dev/null | head -n 1)
fi

if [ -z "$DEB_PKG" ] || [ ! -f "$DEB_PKG" ]; then
    echo "Error: No .deb package found in $REPO_DIR/dist/ for version $VERSION" >&2
    exit 1
fi

# Check if velox is installed on the host system
if dpkg -s velox >/dev/null 2>&1; then
    echo "Existing Velox package detected on the system."
    read -rp "Do you want to uninstall the existing Velox package first? [y/N]: " choice
    if [[ "$choice" =~ ^[Yy]$ ]]; then
        echo "Removing existing Velox package..."
        sudo apt-get remove -y velox
    fi
elif command -v velox >/dev/null 2>&1; then
    BIN_PATH="$(command -v velox)"
    echo "Existing velox binary found at: $BIN_PATH"
    read -rp "Do you want to remove it first? [y/N]: " choice
    if [[ "$choice" =~ ^[Yy]$ ]]; then
        sudo rm -f "$BIN_PATH"
    fi
fi

# Install latest .deb package
echo "Installing $(basename "$DEB_PKG")..."
sudo apt-get install -y "$DEB_PKG"

echo "Velox v$VERSION installed successfully."
