#!/usr/bin/env bash
# ==============================================================================
# Velox Fedora / RHEL RPM Packaging Script
# Builds a binary RPM package (.rpm) using rpmbuild and the velox.spec file.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

PROFILE="optimized-release"
NO_BUILD=0
OUTPUT_DIR="${REPO_ROOT}/dist"

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Builds a Fedora / RHEL binary package (.rpm) for Velox.

Options:
    --profile <PROFILE>     Cargo build profile ('optimized-release' or 'release', default: optimized-release)
    --no-build              Skip compiling; package existing target binary immediately
    --output-dir <DIR>      Directory to write the .rpm package (default: dist/)
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
echo " Packaging Velox v${VERSION} for Fedora / RHEL (${ARCH})"
echo " Profile:    ${PROFILE}"
echo " Output Dir: ${OUTPUT_DIR}"
echo "=================================================="

# Update version in spec file
if [[ -f "${SCRIPT_DIR}/velox.spec" ]]; then
    sed -i "s/^Version:\s*.*/Version:        ${VERSION}/" "${SCRIPT_DIR}/velox.spec"
fi

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

# Setup RPM build root
RPM_ROOT="$(mktemp -d "/tmp/velox-rpm-build-XXXXXX")"
cleanup() {
    rm -rf "${RPM_ROOT}"
}
trap cleanup EXIT

mkdir -p "${RPM_ROOT}"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}

# Create source archive for rpmbuild
SOURCE_TAR="${RPM_ROOT}/SOURCES/v${VERSION}.tar.gz"
echo "==> Creating source tarball for rpmbuild..."
git archive --format=tar.gz --prefix="Velox-${VERSION}/" -o "${SOURCE_TAR}" HEAD 2>/dev/null || {
    tar --exclude="./target" --exclude="./.git" -czf "${SOURCE_TAR}" --transform "s,^\.,Velox-${VERSION}," .
}

cp "${SCRIPT_DIR}/velox.spec" "${RPM_ROOT}/SPECS/velox.spec"

echo "==> Executing rpmbuild..."
rpmbuild --define "_topdir ${RPM_ROOT}" -ba "${RPM_ROOT}/SPECS/velox.spec"

mkdir -p "${OUTPUT_DIR}"
find "${RPM_ROOT}/RPMS" -type f -name "*.rpm" -exec cp {} "${OUTPUT_DIR}/" \;
find "${RPM_ROOT}/SRPMS" -type f -name "*.src.rpm" -exec cp {} "${OUTPUT_DIR}/" \; 2>/dev/null || true

echo "=================================================="
echo " RPM packages generated in: ${OUTPUT_DIR}"
ls -la "${OUTPUT_DIR}"/*.rpm 2>/dev/null || true
echo "=================================================="
echo ""
echo "To install on Fedora / RHEL:"
echo "    sudo dnf install scripts/packaging/fedora/velox-${VERSION}-*.${ARCH}.rpm"
echo "=================================================="
