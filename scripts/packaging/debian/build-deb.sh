#!/usr/bin/env bash
# ==============================================================================
# Velox Debian (.deb) Packaging Script
# Generates a standard Debian binary package (.deb) for Velox.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# Configuration defaults
PROFILE="optimized-release"
NO_BUILD=0
OUTPUT_DIR="${REPO_ROOT}/dist"

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Builds a Debian binary package (.deb) for Velox.

Options:
    --profile <PROFILE>     Cargo build profile ('optimized-release' or 'release', default: optimized-release)
    --no-build              Skip compiling; use existing target binary
    --output-dir <DIR>      Directory to write the .deb package (default: dist/)
    -h, --help              Show this help message
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --profile)
            PROFILE="$2"
            shift 2
            ;;
        --no-build)
            NO_BUILD=1
            shift
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo "Error: Unknown argument: $1" >&2
            print_usage
            exit 1
            ;;
    esac
done

cd "${REPO_ROOT}"

# Read version
if [[ -f "VERSION" ]]; then
    VERSION="$(tr -d '\r\n' < VERSION)"
elif [[ -f "Cargo.toml" ]]; then
    VERSION="$(grep -m1 '^version\s*=' Cargo.toml | cut -d '"' -f2)"
else
    echo "Error: Cannot find VERSION or Cargo.toml file in ${REPO_ROOT}" >&2
    exit 1
fi

if [[ -z "${VERSION}" ]]; then
    echo "Error: Version string is empty" >&2
    exit 1
fi

# Detect architecture
if command -v dpkg >/dev/null 2>&1; then
    DEB_ARCH="$(dpkg --print-architecture)"
else
    UNAME_M="$(uname -m)"
    case "${UNAME_M}" in
        x86_64)  DEB_ARCH="amd64" ;;
        aarch64) DEB_ARCH="arm64" ;;
        armv7l)  DEB_ARCH="armhf" ;;
        riscv64) DEB_ARCH="riscv64" ;;
        *)       DEB_ARCH="${UNAME_M}" ;;
    esac
fi

echo "=================================================="
echo " Packaging Velox v${VERSION} for Debian (${DEB_ARCH})"
echo " Profile:    ${PROFILE}"
echo " Output Dir: ${OUTPUT_DIR}"
echo "=================================================="

BINARY_PATH="${REPO_ROOT}/target/${PROFILE}/velox"

# Ensure binary is built
if [[ "${NO_BUILD}" -eq 0 ]]; then
    echo "==> Building Velox with profile '${PROFILE}'..."
    cargo build --profile "${PROFILE}"
fi

if [[ ! -f "${BINARY_PATH}" ]]; then
    # Fallback check for target/release/velox if profile was release
    if [[ "${PROFILE}" == "release" && -f "${REPO_ROOT}/target/release/velox" ]]; then
        BINARY_PATH="${REPO_ROOT}/target/release/velox"
    else
        echo "Error: Binary not found at ${BINARY_PATH}" >&2
        echo "Please run without --no-build or compile with: cargo build --profile ${PROFILE}" >&2
        exit 1
    fi
fi

# Setup temporary staging directory
STAGE_DIR="$(mktemp -d "/tmp/velox-deb-build-XXXXXX")"
cleanup() {
    rm -rf "${STAGE_DIR}"
}
trap cleanup EXIT

echo "==> Staging files in ${STAGE_DIR}..."

# Destination directories
PKG_ROOT="${STAGE_DIR}/velox_${VERSION}_${DEB_ARCH}"
BIN_DIR="${PKG_ROOT}/usr/bin"
APP_DIR="${PKG_ROOT}/usr/share/applications"
ICON_DIR="${PKG_ROOT}/usr/share/icons/hicolor"
DOC_DIR="${PKG_ROOT}/usr/share/doc/velox"
DEBIAN_DIR="${PKG_ROOT}/DEBIAN"
BASH_COMP_DIR="${PKG_ROOT}/usr/share/bash-completion/completions"
ZSH_COMP_DIR="${PKG_ROOT}/usr/share/zsh/vendor-completions"
FISH_COMP_DIR="${PKG_ROOT}/usr/share/fish/vendor_completions.d"

mkdir -p "${BIN_DIR}" "${APP_DIR}" "${ICON_DIR}" "${DOC_DIR}" "${DEBIAN_DIR}" \
         "${BASH_COMP_DIR}" "${ZSH_COMP_DIR}" "${FISH_COMP_DIR}"

# 1. Install binary
install -m 755 "${BINARY_PATH}" "${BIN_DIR}/velox"
# Strip if not already stripped
if command -v strip >/dev/null 2>&1; then
    strip --strip-unneeded "${BIN_DIR}/velox" 2>/dev/null || true
fi

# 2. Install desktop file
if [[ -f "assets/io.github.lnoxsian.Velox.desktop" ]]; then
    install -m 644 "assets/io.github.lnoxsian.Velox.desktop" "${APP_DIR}/io.github.lnoxsian.Velox.desktop"
else
    echo "Warning: assets/io.github.lnoxsian.Velox.desktop not found" >&2
fi

# 3. Install icons
ICON_SIZES=("16x16" "32x32" "48x48" "64x64" "128x128" "256x256" "512x512" "1024x1024")
for size in "${ICON_SIZES[@]}"; do
    mkdir -p "${ICON_DIR}/${size}/apps"
    if [[ -f "assets/generated_icons/icon_${size}.png" ]]; then
        install -m 644 "assets/generated_icons/icon_${size}.png" "${ICON_DIR}/${size}/apps/io.github.lnoxsian.Velox.png"
    fi
done

mkdir -p "${ICON_DIR}/scalable/apps"
if [[ -f "assets/icons/velox_terminal_icon_final.svg" ]]; then
    install -m 644 "assets/icons/velox_terminal_icon_final.svg" "${ICON_DIR}/scalable/apps/io.github.lnoxsian.Velox.svg"
fi

# 4. Install shell completions
if [[ -f "assets/completions/bash/velox" ]]; then
    install -m 644 "assets/completions/bash/velox" "${BASH_COMP_DIR}/velox"
fi
if [[ -f "assets/completions/zsh/_velox" ]]; then
    install -m 644 "assets/completions/zsh/_velox" "${ZSH_COMP_DIR}/_velox"
fi
if [[ -f "assets/completions/fish/velox.fish" ]]; then
    install -m 644 "assets/completions/fish/velox.fish" "${FISH_COMP_DIR}/velox.fish"
fi

# 5. Install documentation and copyright
if [[ -f "README.md" ]]; then
    install -m 644 "README.md" "${DOC_DIR}/README.md"
fi

cat <<EOF > "${DOC_DIR}/copyright"
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: Velox
Upstream-Contact: lnoxsian <https://github.com/lnoxsian/Velox>
Source: https://github.com/lnoxsian/Velox

Files: *
Copyright: 2025-2026 lnoxsian <https://github.com/lnoxsian/Velox>
License: MIT
 Permission is hereby granted, free of charge, to any person obtaining a copy
 of this software and associated documentation files (the "Software"), to deal
 in the Software without restriction, including without limitation the rights
 to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 copies of the Software, and to permit persons to whom the Software is
 furnished to do so, subject to the following conditions:
 .
 The above copyright notice and this permission notice shall be included in all
 copies or substantial portions of the Software.
 .
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 SOFTWARE.
EOF
chmod 644 "${DOC_DIR}/copyright"

# Calculate Installed-Size in KB
INSTALLED_SIZE="$(du -sk "${PKG_ROOT}/usr" | awk '{print $1}')"

# 5. Create DEBIAN/control
cat <<EOF > "${DEBIAN_DIR}/control"
Package: velox
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${DEB_ARCH}
Essential: no
Installed-Size: ${INSTALLED_SIZE}
Maintainer: lnoxsian <https://github.com/lnoxsian/Velox>
Homepage: https://github.com/lnoxsian/Velox
Depends: libc6 (>= 2.34), libgcc-s1 (>= 3.0)
Recommends: fontconfig, hicolor-icon-theme
Description: Fast, modern, lightweight terminal emulator for Linux
 Velox is a high-performance native Linux terminal emulator written in
 pure Rust featuring a custom multi-threaded software renderer, sub-millisecond
 input latency, truecolor, full Unicode/glyph fallback, and zero C library
 runtime dependencies.
EOF
chmod 644 "${DEBIAN_DIR}/control"

# 6. Create postinst and postrm hooks for icon & desktop cache updates
cat <<'EOF' > "${DEBIAN_DIR}/postinst"
#!/bin/sh
set -e

if [ "$1" = "configure" ]; then
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q /usr/share/applications || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
    fi
fi

exit 0
EOF
chmod 755 "${DEBIAN_DIR}/postinst"

cat <<'EOF' > "${DEBIAN_DIR}/postrm"
#!/bin/sh
set -e

if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q /usr/share/applications || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
    fi
fi

exit 0
EOF
chmod 755 "${DEBIAN_DIR}/postrm"

# Build package
mkdir -p "${OUTPUT_DIR}"
OUTPUT_PKG="${OUTPUT_DIR}/velox_${VERSION}_${DEB_ARCH}.deb"

echo "==> Building .deb package with dpkg-deb..."
if command -v fakeroot >/dev/null 2>&1; then
    fakeroot dpkg-deb --build --root-owner-group "${PKG_ROOT}" "${OUTPUT_PKG}"
else
    dpkg-deb --build --root-owner-group "${PKG_ROOT}" "${OUTPUT_PKG}"
fi

echo "=================================================="
echo " Successfully built: ${OUTPUT_PKG}"
echo " Package Size: $(du -h "${OUTPUT_PKG}" | awk '{print $1}')"
echo " SHA256:       $(sha256sum "${OUTPUT_PKG}" | awk '{print $1}')"
echo "=================================================="
echo ""
echo "To inspect package contents:"
echo "    dpkg-deb -c \"${OUTPUT_PKG}\""
echo ""
echo "To view package metadata:"
echo "    dpkg-deb -I \"${OUTPUT_PKG}\""
echo ""
echo "To install on Debian/Ubuntu/Mint:"
echo "    sudo dpkg -i \"${OUTPUT_PKG}\""
echo "=================================================="
