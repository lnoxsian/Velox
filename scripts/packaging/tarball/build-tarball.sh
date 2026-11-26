#!/usr/bin/env bash
# ==============================================================================
# Velox Standalone Binary Tarball (.tar.gz) Packaging Script
# Packages pre-compiled or freshly compiled Velox binaries into a standalone
# distributable tarball with self-contained installer, icons, completions, and docs.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

PROFILE="release"
NO_BUILD=0
OUTPUT_DIR="${REPO_ROOT}/dist"
BINARY_PATH=""
TARGET_TRIPLE=""
VERSION_OVERRIDE=""

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Builds a standalone binary tarball (.tar.gz) for Velox.

Options:
    --binary <PATH>         Path to pre-compiled velox binary (skips cargo build)
    --target <TRIPLE>       Target triple name (e.g., x86_64-unknown-linux-gnu)
    --version <VERSION>     Package version override (default: read from VERSION file)
    --profile <PROFILE>     Cargo build profile ('release' or 'debug', default: release)
    --no-build              Skip compiling; package existing target binary immediately
    --output-dir <DIR>      Directory to write the tarball archive (default: dist/)
    -h, --help              Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --binary)
            BINARY_PATH="$2"
            NO_BUILD=1
            shift 2
            ;;
        --target)
            TARGET_TRIPLE="$2"
            shift 2
            ;;
        --version)
            VERSION_OVERRIDE="$2"
            shift 2
            ;;
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
if [[ -n "${VERSION_OVERRIDE}" ]]; then
    VERSION="${VERSION_OVERRIDE}"
elif [[ -f "VERSION" ]]; then
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

if [[ -z "${TARGET_TRIPLE}" ]]; then
    UNAME_M="$(uname -m)"
    case "${UNAME_M}" in
        x86_64)  TARGET_TRIPLE="x86_64-unknown-linux-gnu" ;;
        aarch64) TARGET_TRIPLE="aarch64-unknown-linux-gnu" ;;
        armv7l)  TARGET_TRIPLE="armv7-unknown-linux-gnueabihf" ;;
        i686)    TARGET_TRIPLE="i686-unknown-linux-gnu" ;;
        *)       TARGET_TRIPLE="${UNAME_M}-unknown-linux-gnu" ;;
    esac
fi

echo "=================================================="
echo " Packaging Velox v${VERSION} Tarball (${TARGET_TRIPLE})"
echo " Output Dir: ${OUTPUT_DIR}"
echo "=================================================="

if [[ -z "${BINARY_PATH}" ]]; then
    BINARY_PATH="${REPO_ROOT}/target/${PROFILE}/velox"
    if [[ "${NO_BUILD}" -eq 0 ]]; then
        echo "==> Building Velox with profile '${PROFILE}'..."
        cargo build --profile "${PROFILE}" --bins
    fi
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
STAGE_DIR="$(mktemp -d "/tmp/velox-tarball-build-XXXXXX")"
cleanup() {
    rm -rf "${STAGE_DIR}"
}
trap cleanup EXIT

BUNDLE_NAME="velox-v${VERSION}-${TARGET_TRIPLE}"
BUNDLE_DIR="${STAGE_DIR}/${BUNDLE_NAME}"
mkdir -p "${BUNDLE_DIR}"
mkdir -p "${BUNDLE_DIR}/assets"

# 1. Install binaries
install -m 755 "${BINARY_PATH}" "${BUNDLE_DIR}/velox"

BINARY_DIR="$(dirname "${BINARY_PATH}")"
if [[ -f "${BINARY_DIR}/veloxcfg" ]]; then
    install -m 755 "${BINARY_DIR}/veloxcfg" "${BUNDLE_DIR}/veloxcfg"
elif [[ -f "${REPO_ROOT}/target/${PROFILE}/veloxcfg" ]]; then
    install -m 755 "${REPO_ROOT}/target/${PROFILE}/veloxcfg" "${BUNDLE_DIR}/veloxcfg"
fi

if command -v strip >/dev/null 2>&1; then
    strip --strip-unneeded "${BUNDLE_DIR}/velox" 2>/dev/null || true
    if [[ -f "${BUNDLE_DIR}/veloxcfg" ]]; then
        strip --strip-unneeded "${BUNDLE_DIR}/veloxcfg" 2>/dev/null || true
    fi
fi

# 2. Copy documentation and license
if [[ -f "README.md" ]]; then
    install -m 644 "README.md" "${BUNDLE_DIR}/README.md"
fi
if [[ -f "LICENSE" ]]; then
    install -m 644 "LICENSE" "${BUNDLE_DIR}/LICENSE"
fi
if [[ -f "VERSION" ]]; then
    install -m 644 "VERSION" "${BUNDLE_DIR}/VERSION"
fi

# 3. Copy asset bundle (desktop file, icons, completions)
if [[ -d "assets" ]]; then
    cp -r assets "${BUNDLE_DIR}/"
fi

# 4. Generate self-contained install script
cat <<'EOF' > "${BUNDLE_DIR}/install.sh"
#!/usr/bin/env bash
# ==============================================================================
# Velox Standalone Installer
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-/usr/local}"
DESTDIR="${DESTDIR:-}"

BINDIR="${DESTDIR}${PREFIX}/bin"
DATADIR="${DESTDIR}${PREFIX}/share"
APPDIR="${DATADIR}/applications"
ICONDIR="${DATADIR}/icons/hicolor"
BASHCOMPDIR="${DATADIR}/bash-completion/completions"
ZSHCOMPDIR="${DATADIR}/zsh/site-functions"
FISHCOMPDIR="${DATADIR}/fish/vendor_completions.d"

echo "==> Installing Velox to ${PREFIX}..."

install -d "${BINDIR}"
install -m 755 "${SCRIPT_DIR}/velox" "${BINDIR}/velox"
if [[ -f "${SCRIPT_DIR}/veloxcfg" ]]; then
    install -m 755 "${SCRIPT_DIR}/veloxcfg" "${BINDIR}/veloxcfg"
fi

# Desktop Entry
if [[ -f "${SCRIPT_DIR}/assets/io.github.lnoxsian.Velox.desktop" ]]; then
    install -d "${APPDIR}"
    install -m 644 "${SCRIPT_DIR}/assets/io.github.lnoxsian.Velox.desktop" "${APPDIR}/io.github.lnoxsian.Velox.desktop"
fi

# Icons
ICON_SIZES=("16x16" "32x32" "48x48" "64x64" "128x128" "256x256" "512x512" "1024x1024")
for size in "${ICON_SIZES[@]}"; do
    if [[ -f "${SCRIPT_DIR}/assets/generated_icons/icon_${size}.png" ]]; then
        install -d "${ICONDIR}/${size}/apps"
        install -m 644 "${SCRIPT_DIR}/assets/generated_icons/icon_${size}.png" "${ICONDIR}/${size}/apps/io.github.lnoxsian.Velox.png"
    fi
done

if [[ -f "${SCRIPT_DIR}/assets/icons/velox_terminal_icon_final.svg" ]]; then
    install -d "${ICONDIR}/scalable/apps"
    install -m 644 "${SCRIPT_DIR}/assets/icons/velox_terminal_icon_final.svg" "${ICONDIR}/scalable/apps/io.github.lnoxsian.Velox.svg"
fi

# Shell completions
if [[ -f "${SCRIPT_DIR}/assets/completions/bash/velox" ]]; then
    install -d "${BASHCOMPDIR}"
    install -m 644 "${SCRIPT_DIR}/assets/completions/bash/velox" "${BASHCOMPDIR}/velox"
fi
if [[ -f "${SCRIPT_DIR}/assets/completions/zsh/_velox" ]]; then
    install -d "${ZSHCOMPDIR}"
    install -m 644 "${SCRIPT_DIR}/assets/completions/zsh/_velox" "${ZSHCOMPDIR}/_velox"
fi
if [[ -f "${SCRIPT_DIR}/assets/completions/fish/velox.fish" ]]; then
    install -d "${FISHCOMPDIR}"
    install -m 644 "${SCRIPT_DIR}/assets/completions/fish/velox.fish" "${FISHCOMPDIR}/velox.fish"
fi

# Caches
if [[ -z "${DESTDIR}" ]]; then
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q "${APPDIR}" 2>/dev/null || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t -f "${ICONDIR}" 2>/dev/null || true
    fi
fi

echo "==> Velox installed successfully!"
echo "    Binary: ${BINDIR}/velox"
EOF
chmod 755 "${BUNDLE_DIR}/install.sh"

# 5. Generate uninstall script
cat <<'EOF' > "${BUNDLE_DIR}/uninstall.sh"
#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
DESTDIR="${DESTDIR:-}"

BINDIR="${DESTDIR}${PREFIX}/bin"
DATADIR="${DESTDIR}${PREFIX}/share"
APPDIR="${DATADIR}/applications"
ICONDIR="${DATADIR}/icons/hicolor"
BASHCOMPDIR="${DATADIR}/bash-completion/completions"
ZSHCOMPDIR="${DATADIR}/zsh/site-functions"
FISHCOMPDIR="${DATADIR}/fish/vendor_completions.d"

echo "==> Uninstalling Velox from ${PREFIX}..."
rm -f "${BINDIR}/velox"
rm -f "${BINDIR}/veloxcfg"
rm -f "${APPDIR}/io.github.lnoxsian.Velox.desktop"
for size in 16x16 32x32 48x48 64x64 128x128 256x256 512x512 1024x1024; do
    rm -f "${ICONDIR}/${size}/apps/io.github.lnoxsian.Velox.png"
done
rm -f "${ICONDIR}/scalable/apps/io.github.lnoxsian.Velox.svg"
rm -f "${BASHCOMPDIR}/velox"
rm -f "${ZSHCOMPDIR}/_velox"
rm -f "${FISHCOMPDIR}/velox.fish"

if [[ -z "${DESTDIR}" ]]; then
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q "${APPDIR}" 2>/dev/null || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t -f "${ICONDIR}" 2>/dev/null || true
    fi
fi

echo "==> Velox uninstalled successfully."
EOF
chmod 755 "${BUNDLE_DIR}/uninstall.sh"

mkdir -p "${OUTPUT_DIR}"
OUTPUT_DIR="$(cd "${OUTPUT_DIR}" && pwd)"
ARCHIVE_PATH="${OUTPUT_DIR}/${BUNDLE_NAME}.tar.gz"

echo "==> Creating tarball: ${ARCHIVE_PATH}..."
tar -czf "${ARCHIVE_PATH}" -C "${STAGE_DIR}" "${BUNDLE_NAME}"

SHA256="$(sha256sum "${ARCHIVE_PATH}" | awk '{print $1}')"
echo "${SHA256}  $(basename "${ARCHIVE_PATH}")" > "${ARCHIVE_PATH}.sha256"

echo "=================================================="
echo " Successfully built: ${ARCHIVE_PATH}"
echo " Package Size: $(du -h "${ARCHIVE_PATH}" | awk '{print $1}')"
echo " SHA256:       ${SHA256}"
echo "=================================================="
