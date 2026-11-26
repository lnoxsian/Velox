#!/usr/bin/env bash
# ==============================================================================
# Velox Fedora / RHEL RPM Packaging Script
# Builds a binary RPM package (.rpm) using rpmbuild.
# Supports native builds, cross-compiled pre-built binaries, GNU & Musl targets.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

PROFILE="release"
NO_BUILD=0
OUTPUT_DIR="${REPO_ROOT}/dist"
BINARY_PATH=""
ARCH_OVERRIDE=""
VERSION_OVERRIDE=""
LIBC="gnu"

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Builds a Fedora / RHEL binary package (.rpm) for Velox.

Options:
    --binary <PATH>         Path to pre-compiled velox binary (skips cargo build)
    --arch <ARCH>           Target RPM architecture (e.g., x86_64, aarch64, armv7hl, i686)
    --version <VERSION>     Package version override (default: read from VERSION file)
    --libc <gnu|musl>       C library runtime flavor (default: gnu)
    --profile <PROFILE>     Cargo build profile ('release' or 'debug', default: release)
    --no-build              Skip compiling; package existing target binary immediately
    --output-dir <DIR>      Directory to write the .rpm package (default: dist/)
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
        --arch)
            ARCH_OVERRIDE="$2"
            shift 2
            ;;
        --version)
            VERSION_OVERRIDE="$2"
            shift 2
            ;;
        --libc)
            LIBC="$2"
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

# Check for rpmbuild
if ! command -v rpmbuild >/dev/null 2>&1; then
    echo "Error: 'rpmbuild' is required but not installed." >&2
    echo "Install it via: sudo apt-get install -y rpm (Debian/Ubuntu) or sudo dnf install -y rpm-build (Fedora/RHEL)" >&2
    exit 1
fi

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

if [[ -n "${ARCH_OVERRIDE}" ]]; then
    RPM_ARCH="${ARCH_OVERRIDE}"
else
    UNAME_M="$(uname -m)"
    case "${UNAME_M}" in
        x86_64)          RPM_ARCH="x86_64" ;;
        aarch64|arm64)   RPM_ARCH="aarch64" ;;
        armv7l|armhf)    RPM_ARCH="armv7hl" ;;
        i386|i686)       RPM_ARCH="i686" ;;
        *)               RPM_ARCH="${UNAME_M}" ;;
    esac
fi

echo "=================================================="
echo " Packaging Velox v${VERSION} for Fedora / RHEL (${RPM_ARCH}, libc: ${LIBC})"
echo " Profile:    ${PROFILE}"
echo " Output Dir: ${OUTPUT_DIR}"
echo "=================================================="

# Setup RPM build root
RPM_ROOT="$(mktemp -d "/tmp/velox-rpm-build-XXXXXX")"
cleanup() {
    rm -rf "${RPM_ROOT}"
}
trap cleanup EXIT

mkdir -p "${RPM_ROOT}"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}

# If a binary is specified or compiled binary should be packaged directly
if [[ -n "${BINARY_PATH}" || "${NO_BUILD}" -eq 1 ]]; then
    if [[ -z "${BINARY_PATH}" ]]; then
        BINARY_PATH="${REPO_ROOT}/target/${PROFILE}/velox"
    fi
    if [[ ! -f "${BINARY_PATH}" ]]; then
        echo "Error: Binary not found at ${BINARY_PATH}" >&2
        exit 1
    fi

    # Stage files for packaging
    STAGE_DIR="${RPM_ROOT}/STAGE"
    mkdir -p "${STAGE_DIR}/usr/bin"
    mkdir -p "${STAGE_DIR}/usr/share/applications"
    mkdir -p "${STAGE_DIR}/usr/share/icons/hicolor"
    mkdir -p "${STAGE_DIR}/usr/share/licenses/velox"
    mkdir -p "${STAGE_DIR}/usr/share/doc/velox"
    mkdir -p "${STAGE_DIR}/usr/share/bash-completion/completions"
    mkdir -p "${STAGE_DIR}/usr/share/zsh/site-functions"
    mkdir -p "${STAGE_DIR}/usr/share/fish/vendor_completions.d"

    # Install binary
    install -m 755 "${BINARY_PATH}" "${STAGE_DIR}/usr/bin/velox"
    
    # Check if veloxcfg exists
    BINARY_DIR="$(dirname "${BINARY_PATH}")"
    HAS_VELOXCFG=0
    if [[ -f "${BINARY_DIR}/veloxcfg" ]]; then
        install -m 755 "${BINARY_DIR}/veloxcfg" "${STAGE_DIR}/usr/bin/veloxcfg"
        HAS_VELOXCFG=1
    elif [[ -f "${REPO_ROOT}/target/${PROFILE}/veloxcfg" ]]; then
        install -m 755 "${REPO_ROOT}/target/${PROFILE}/veloxcfg" "${STAGE_DIR}/usr/bin/veloxcfg"
        HAS_VELOXCFG=1
    fi

    # Strip if native host strip supports target
    if command -v strip >/dev/null 2>&1; then
        strip --strip-unneeded "${STAGE_DIR}/usr/bin/velox" 2>/dev/null || true
        if [[ "${HAS_VELOXCFG}" -eq 1 ]]; then
            strip --strip-unneeded "${STAGE_DIR}/usr/bin/veloxcfg" 2>/dev/null || true
        fi
    fi

    # Desktop entry
    if [[ -f "assets/io.github.lnoxsian.Velox.desktop" ]]; then
        install -m 644 "assets/io.github.lnoxsian.Velox.desktop" "${STAGE_DIR}/usr/share/applications/io.github.lnoxsian.Velox.desktop"
    fi

    # Icons
    ICON_SIZES=("16x16" "32x32" "48x48" "64x64" "128x128" "256x256" "512x512" "1024x1024")
    for size in "${ICON_SIZES[@]}"; do
        mkdir -p "${STAGE_DIR}/usr/share/icons/hicolor/${size}/apps"
        if [[ -f "assets/generated_icons/icon_${size}.png" ]]; then
            install -m 644 "assets/generated_icons/icon_${size}.png" "${STAGE_DIR}/usr/share/icons/hicolor/${size}/apps/io.github.lnoxsian.Velox.png"
        fi
    done
    mkdir -p "${STAGE_DIR}/usr/share/icons/hicolor/scalable/apps"
    if [[ -f "assets/icons/velox_terminal_icon_final.svg" ]]; then
        install -m 644 "assets/icons/velox_terminal_icon_final.svg" "${STAGE_DIR}/usr/share/icons/hicolor/scalable/apps/io.github.lnoxsian.Velox.svg"
    fi

    # Completions
    if [[ -f "assets/completions/bash/velox" ]]; then
        install -m 644 "assets/completions/bash/velox" "${STAGE_DIR}/usr/share/bash-completion/completions/velox"
    fi
    if [[ -f "assets/completions/zsh/_velox" ]]; then
        install -m 644 "assets/completions/zsh/_velox" "${STAGE_DIR}/usr/share/zsh/site-functions/_velox"
    fi
    if [[ -f "assets/completions/fish/velox.fish" ]]; then
        install -m 644 "assets/completions/fish/velox.fish" "${STAGE_DIR}/usr/share/fish/vendor_completions.d/velox.fish"
    fi

    # Documentation and License
    if [[ -f "README.md" ]]; then
        install -m 644 "README.md" "${STAGE_DIR}/usr/share/doc/velox/README.md"
    fi
    if [[ -f "LICENSE" ]]; then
        install -m 644 "LICENSE" "${STAGE_DIR}/usr/share/licenses/velox/LICENSE"
    fi

    # Create binary RPM spec file
    SPEC_FILE="${RPM_ROOT}/SPECS/velox-bin.spec"
    if [[ "${LIBC}" == "musl" ]]; then
        RELEASE_TAG="1.musl"
        REQ_LINES="Recommends:     fontconfig\nRecommends:     hicolor-icon-theme"
    else
        RELEASE_TAG="1"
        REQ_LINES="Requires:       glibc\nRequires:       libgcc\nRecommends:     fontconfig\nRecommends:     hicolor-icon-theme"
    fi

    cat <<EOF > "${SPEC_FILE}"
Name:           velox
Version:        ${VERSION}
Release:        ${RELEASE_TAG}%{?dist}
Summary:        Fast, modern, lightweight terminal emulator for Linux
License:        MIT
URL:            https://github.com/lnoxsian/Velox

%define debug_package %{nil}
%define _binaries_in_noarch_packages_terminate_build 0
%define _build_id_links none
%define __strip /bin/true
%define __os_install_post %{nil}
AutoReqProv:    no

$(echo -e "${REQ_LINES}")

%description
Velox is a high-performance native Linux terminal emulator written in
pure Rust featuring a custom multi-threaded software renderer, sub-millisecond
input latency, truecolor, full Unicode/glyph fallback, and zero C library
runtime dependencies.

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
cp -a ${STAGE_DIR}/* %{buildroot}/

%post
/bin/touch --no-create %{_datadir}/icons/hicolor &>/dev/null || :
/usr/bin/update-desktop-database &>/dev/null || :

%postun
if [ \$1 -eq 0 ] ; then
    /bin/touch --no-create %{_datadir}/icons/hicolor &>/dev/null
    /usr/bin/gtk-update-icon-cache %{_datadir}/icons/hicolor &>/dev/null || :
    /usr/bin/update-desktop-database &>/dev/null || :
fi

%posttrans
/usr/bin/gtk-update-icon-cache %{_datadir}/icons/hicolor &>/dev/null || :

%files
%license /usr/share/licenses/velox/LICENSE
%doc /usr/share/doc/velox/README.md
%{_bindir}/velox
EOF

    if [[ "${HAS_VELOXCFG}" -eq 1 ]]; then
        echo "%{_bindir}/veloxcfg" >> "${SPEC_FILE}"
    fi

    cat <<EOF >> "${SPEC_FILE}"
%{_datadir}/applications/io.github.lnoxsian.Velox.desktop
%{_datadir}/icons/hicolor/*/apps/io.github.lnoxsian.Velox.*
%{_datadir}/bash-completion/completions/velox
%{_datadir}/zsh/site-functions/_velox
%{_datadir}/fish/vendor_completions.d/velox.fish
EOF

    echo "==> Executing rpmbuild for target ${RPM_ARCH}..."
    rpmbuild --define "_topdir ${RPM_ROOT}" --target "${RPM_ARCH}" -bb "${SPEC_FILE}"
else
    # Native compile workflow via source rpm spec
    if [[ -f "${SCRIPT_DIR}/velox.spec" ]]; then
        sed -i "s/^Version:\s*.*/Version:        ${VERSION}/" "${SCRIPT_DIR}/velox.spec"
    fi

    BINARY_PATH="${REPO_ROOT}/target/${PROFILE}/velox"
    echo "==> Building Velox with profile '${PROFILE}'..."
    cargo build --profile "${PROFILE}" --bins

    # Create source archive for rpmbuild
    SOURCE_TAR="${RPM_ROOT}/SOURCES/v${VERSION}.tar.gz"
    echo "==> Creating source tarball for rpmbuild..."
    git archive --format=tar.gz --prefix="Velox-${VERSION}/" -o "${SOURCE_TAR}" HEAD 2>/dev/null || {
        tar --exclude="./target" --exclude="./.git" -czf "${SOURCE_TAR}" --transform "s,^\.,Velox-${VERSION}," .
    }

    cp "${SCRIPT_DIR}/velox.spec" "${RPM_ROOT}/SPECS/velox.spec"

    echo "==> Executing rpmbuild..."
    rpmbuild --define "_topdir ${RPM_ROOT}" --target "${RPM_ARCH}" -ba "${RPM_ROOT}/SPECS/velox.spec"
fi

mkdir -p "${OUTPUT_DIR}"
OUTPUT_DIR="$(cd "${OUTPUT_DIR}" && pwd)"
find "${RPM_ROOT}/RPMS" -type f -name "*.rpm" -exec cp -f {} "${OUTPUT_DIR}/" \;
find "${RPM_ROOT}/SRPMS" -type f -name "*.src.rpm" -exec cp -f {} "${OUTPUT_DIR}/" \; 2>/dev/null || true

echo "=================================================="
echo " RPM packages generated in: ${OUTPUT_DIR}"
ls -la "${OUTPUT_DIR}"/*.rpm 2>/dev/null || true
echo "=================================================="
echo ""
echo "To install on Fedora / RHEL:"
echo "    sudo dnf install ${OUTPUT_DIR}/velox-${VERSION}-*.${RPM_ARCH}.rpm"
echo "=================================================="
