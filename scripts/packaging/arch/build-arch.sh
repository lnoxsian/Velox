#!/usr/bin/env bash
# ==============================================================================
# Velox Arch Linux Packaging Script
# Builds an Arch Linux package (.pkg.tar.zst) using makepkg (if available) or
# directly generates a pacman-compatible package archive using tar and zstd.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

PROFILE="optimized-release"
NO_BUILD=0
STANDALONE=0
OUTPUT_DIR="${REPO_ROOT}/dist"

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Builds an Arch Linux package (.pkg.tar.zst) for Velox.

Options:
    --profile <PROFILE>     Cargo build profile ('optimized-release' or 'release', default: optimized-release)
    --no-build              Skip compiling; package existing target binary immediately
    --standalone            Force standalone packaging (using tar/zstd without calling makepkg)
    --output-dir <DIR>      Output directory for package archive (default: dist/)
    -h, --help              Show this help message
EOF
}

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
        --standalone)
            STANDALONE=1
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

ARCH="$(uname -m)"

echo "=================================================="
echo " Packaging Velox v${VERSION} for Arch Linux (${ARCH})"
echo " Profile:    ${PROFILE}"
echo " Output Dir: ${OUTPUT_DIR}"
echo "=================================================="

# Update pkgver in PKGBUILD if it doesn't match
if [[ -f "${SCRIPT_DIR}/PKGBUILD" ]]; then
    sed -i "s/^pkgver=.*/pkgver=${VERSION}/" "${SCRIPT_DIR}/PKGBUILD"
fi

# If makepkg is available and standalone mode is not forced, use makepkg
if command -v makepkg >/dev/null 2>&1 && [[ "${STANDALONE}" -eq 0 ]]; then
    echo "==> makepkg found on system. Building via makepkg..."
    cd "${SCRIPT_DIR}"
    makepkg -f -p PKGBUILD
    mkdir -p "${OUTPUT_DIR}"
    find "${SCRIPT_DIR}" -maxdepth 1 -name "*.pkg.tar.*" -exec mv -f {} "${OUTPUT_DIR}/" \;
    echo "==> Package built successfully with makepkg and placed in ${OUTPUT_DIR}."
    exit 0
fi

echo "==> Building Arch package using standalone packager (tar + zstd)..."

BINARY_PATH="${REPO_ROOT}/target/${PROFILE}/velox"

if [[ "${NO_BUILD}" -eq 0 ]]; then
    echo "==> Building Velox with profile '${PROFILE}'..."
    cargo build --profile "${PROFILE}"
fi

if [[ ! -f "${BINARY_PATH}" ]]; then
    if [[ "${PROFILE}" == "release" && -f "${REPO_ROOT}/target/release/velox" ]]; then
        BINARY_PATH="${REPO_ROOT}/target/release/velox"
    else
        echo "Error: Binary not found at ${BINARY_PATH}" >&2
        exit 1
    fi
fi

# Setup staging directory
STAGE_DIR="$(mktemp -d "/tmp/velox-arch-build-XXXXXX")"
cleanup() {
    rm -rf "${STAGE_DIR}"
}
trap cleanup EXIT

PKG_DIR="${STAGE_DIR}/pkg"
mkdir -p "${PKG_DIR}/usr/bin"
mkdir -p "${PKG_DIR}/usr/share/applications"
mkdir -p "${PKG_DIR}/usr/share/icons/hicolor"
mkdir -p "${PKG_DIR}/usr/share/licenses/velox"
mkdir -p "${PKG_DIR}/usr/share/doc/velox"

# 1. Binary
install -m 755 "${BINARY_PATH}" "${PKG_DIR}/usr/bin/velox"
if command -v strip >/dev/null 2>&1; then
    strip --strip-unneeded "${PKG_DIR}/usr/bin/velox" 2>/dev/null || true
fi

# 2. Desktop file
if [[ -f "assets/io.github.lnoxsian.Velox.desktop" ]]; then
    install -m 644 "assets/io.github.lnoxsian.Velox.desktop" "${PKG_DIR}/usr/share/applications/io.github.lnoxsian.Velox.desktop"
fi

# 3. Icons
ICON_SIZES=("16x16" "32x32" "48x48" "64x64" "128x128" "256x256" "512x512" "1024x1024")
for size in "${ICON_SIZES[@]}"; do
    mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/${size}/apps"
    if [[ -f "assets/generated_icons/icon_${size}.png" ]]; then
        install -m 644 "assets/generated_icons/icon_${size}.png" "${PKG_DIR}/usr/share/icons/hicolor/${size}/apps/io.github.lnoxsian.Velox.png"
    fi
done

mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps"
if [[ -f "assets/icons/velox_terminal_icon_final.svg" ]]; then
    install -m 644 "assets/icons/velox_terminal_icon_final.svg" "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps/io.github.lnoxsian.Velox.svg"
fi

# 4. License and Documentation
if [[ -f "LICENSE" ]]; then
    install -m 644 "LICENSE" "${PKG_DIR}/usr/share/licenses/velox/LICENSE"
fi
if [[ -f "README.md" ]]; then
    install -m 644 "README.md" "${PKG_DIR}/usr/share/doc/velox/README.md"
fi

# Calculate installed package size in bytes
INSTALLED_BYTES="$(du -sb "${PKG_DIR}" | awk '{print $1}')"
BUILD_DATE="$(date +%s)"

# 5. Generate .PKGINFO
cat <<EOF > "${PKG_DIR}/.PKGINFO"
pkgname = velox
pkgbase = velox
pkgver = ${VERSION}-1
pkgdesc = Fast, modern, lightweight terminal emulator for Linux
url = https://github.com/lnoxsian/Velox
builddate = ${BUILD_DATE}
packager = lnoxsian <https://github.com/lnoxsian/Velox>
size = ${INSTALLED_BYTES}
arch = ${ARCH}
license = MIT
depend = glibc
depend = gcc-libs
optdepend = fontconfig: system font discovery
optdepend = hicolor-icon-theme: proper desktop icon integration
provides = velox
conflict = velox-git
conflict = velox-bin
EOF

mkdir -p "${OUTPUT_DIR}"
OUTPUT_PKG="${OUTPUT_DIR}/velox-${VERSION}-1-${ARCH}.pkg.tar.zst"

echo "==> Creating Arch package archive (.pkg.tar.zst)..."
if command -v zstd >/dev/null 2>&1; then
    (cd "${PKG_DIR}" && tar -cf - .PKGINFO usr | zstd -T0 -19 -o "${OUTPUT_PKG}")
else
    # Fallback to .xz if zstd is unavailable
    OUTPUT_PKG="${OUTPUT_DIR}/velox-${VERSION}-1-${ARCH}.pkg.tar.xz"
    (cd "${PKG_DIR}" && tar -cJf "${OUTPUT_PKG}" .PKGINFO usr)
fi

echo "=================================================="
echo " Successfully built: ${OUTPUT_PKG}"
echo " Package Size: $(du -h "${OUTPUT_PKG}" | awk '{print $1}')"
echo " SHA256:       $(sha256sum "${OUTPUT_PKG}" | awk '{print $1}')"
echo "=================================================="
echo ""
echo "To inspect package contents:"
echo "    tar -tf \"${OUTPUT_PKG}\""
echo ""
echo "To install on Arch Linux:"
echo "    sudo pacman -U \"${OUTPUT_PKG}\""
echo "=================================================="
