#!/usr/bin/env bash
# ==============================================================================
# Velox Terminal Emulator - Developer Environment Bootstrap Script
# ==============================================================================
# Installs and validates all development tools, compilers, desktop integration
# utilities, font packages, Python dependencies, asset generators, profiling tools,
# and packaging utilities needed to build, test, optimize, package, and install Velox.
#
# Note: Velox is written in pure Rust and requires ZERO C graphics/windowing/keyboard
# library runtime or development dependencies (pure-Rust X11 via x11rb, pure-Rust
# Wayland via wayland-client, and pure-Rust keyboard mapping via kbvm).
#
# Supported Distributions:
#   - Debian, Ubuntu, Linux Mint, Pop!_OS (apt)
#   - Fedora, RHEL, CentOS, Rocky Linux (dnf / yum)
#   - Arch Linux, Manjaro, EndeavourOS (pacman)
#   - openSUSE Leap / Tumbleweed (zypper)
#   - Alpine Linux (apk)
#   - Void Linux (xbps)
#
# Usage:
#   bash scripts/bootstrap_velox.sh [OPTIONS]
#
# Options:
#   -y, --yes      Non-interactive mode (automatically proceed with installation)
#   -c, --check    Check status of dependencies without installing
#   -d, --dry-run  Print package manager commands without executing them
#   -h, --help     Show this help message
# ==============================================================================

set -euo pipefail

# ANSI color codes
BOLD=$'\e[1m'
CYAN=$'\e[1;36m'
GREEN=$'\e[1;32m'
YELLOW=$'\e[1;33m'
RED=$'\e[1;31m'
BLUE=$'\e[1;34m'
NC=$'\e[0m' # No Color

# Default options
ASSUME_YES=0
CHECK_ONLY=0
DRY_RUN=0

print_banner() {
    cat <<EOF
${CYAN}==============================================================================
                 
                 ┌──────────────────────────────────────────┐
                 | VELOX               VELOX                |
                 | VELOX               VELOX                |
                 | VELOX               VELOX                |
                 |      VELOX     VELOX     VELOX           |
                 |      VELOX     VELOX     VELOX           |
                 |      VELOX     VELOX     VELOX           |
                 |      VELOX     VELOX     VELOX           |
                 |      VELOX     VELOX     VELOX           |
                 |      VELOX     VELOX     VELOX           |
                 |           VELOX               VELOXVELOX |
                 |           VELOX               VELOXVELOX |
                 |           VELOX               VELOXVELOX |
                 └──────────────────────────────────────────┘
             
                    Velox Development Environment Bootstrap
==============================================================================${NC}
EOF
}

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Installs all development tools, libraries, and utilities needed for Velox.

Options:
    -y, --yes       Non-interactive mode; assume yes to all prompts
    -c, --check     Check status of dependencies without installing anything
    -d, --dry-run   Print installation commands without modifying the system
    -h, --help      Show this help message

Examples:
    make bootstrap
    bash scripts/bootstrap_velox.sh --check
    bash scripts/bootstrap_velox.sh -y
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        -y|--yes)
            ASSUME_YES=1
            shift
            ;;
        -c|--check)
            CHECK_ONLY=1
            shift
            ;;
        -d|--dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo -e "${RED}[ERROR]${NC} Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Ensure ~/.cargo/bin is in PATH if present
if [[ -d "$HOME/.cargo/bin" ]] && [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# Detect Linux Distribution and Package Manager
detect_distro() {
    DISTRO_ID="unknown"
    PKG_MGR="unknown"

    if [[ -f /etc/os-release ]]; then
        # shellcheck disable=SC1091
        source /etc/os-release
        DISTRO_ID="${ID:-unknown}"
        DISTRO_LIKE="${ID_LIKE:-}"
    else
        DISTRO_ID="$(uname -s | tr '[:upper:]' '[:lower:]')"
        DISTRO_LIKE=""
    fi

    if command -v apt-get &>/dev/null; then
        PKG_MGR="apt"
    elif command -v dnf &>/dev/null; then
        PKG_MGR="dnf"
    elif command -v yum &>/dev/null; then
        PKG_MGR="yum"
    elif command -v pacman &>/dev/null; then
        PKG_MGR="pacman"
    elif command -v zypper &>/dev/null; then
        PKG_MGR="zypper"
    elif command -v apk &>/dev/null; then
        PKG_MGR="apk"
    elif command -v xbps-install &>/dev/null; then
        PKG_MGR="xbps"
    fi
}

# Check if a package is installed via system package manager
is_pkg_installed() {
    local pkg="$1"
    case "$PKG_MGR" in
        apt)
            dpkg -s "$pkg" &>/dev/null
            ;;
        dnf|yum|zypper)
            rpm -q "$pkg" &>/dev/null
            ;;
        pacman)
            pacman -Qi "$pkg" &>/dev/null
            ;;
        apk)
            apk info -e "$pkg" &>/dev/null
            ;;
        xbps)
            xbps-query -s "$pkg" &>/dev/null
            ;;
        *)
            false
            ;;
    esac
}

# Define categorized packages per package manager
get_packages_for_manager() {
    case "$PKG_MGR" in
        apt)
            CORE_PKGS=(
                "build-essential"
                "pkg-config"
                "make"
                "gcc"
                "binutils"
            )
            DESKTOP_PKGS=(
                "desktop-file-utils"
                "gtk-update-icon-cache"
                "hicolor-icon-theme"
                "shared-mime-info"
                "xdg-utils"
                "libglib2.0-bin"
            )
            FONT_PKGS=(
                "fontconfig"
                "fonts-dejavu-core"
                "fonts-noto-color-emoji"
            )
            PYTHON_PKGS=(
                "python3"
                "python3-pip"
                "python3-pil"
                "librsvg2-bin"
                "optipng"
                "pngquant"
            )
            BENCH_PKGS=(
                "util-linux"
                "bc"
                "procps"
            )
            PACKAGING_PKGS=(
                "dpkg-dev"
                "fakeroot"
                "zstd"
                "tar"
                "xz-utils"
            )
            ;;

        dnf|yum)
            CORE_PKGS=(
                "gcc"
                "gcc-c++"
                "make"
                "pkgconf-pkg-config"
                "binutils"
            )
            DESKTOP_PKGS=(
                "desktop-file-utils"
                "gtk3"
                "hicolor-icon-theme"
                "shared-mime-info"
                "xdg-utils"
                "glib2"
            )
            FONT_PKGS=(
                "fontconfig"
                "dejavu-sans-mono-fonts"
                "google-noto-color-emoji-fonts"
            )
            PYTHON_PKGS=(
                "python3"
                "python3-pip"
                "python3-pillow"
                "librsvg2-tools"
                "optipng"
                "pngquant"
            )
            BENCH_PKGS=(
                "util-linux"
                "bc"
                "procps-ng"
            )
            PACKAGING_PKGS=(
                "rpm-build"
                "zstd"
                "tar"
                "xz"
            )
            ;;

        pacman)
            CORE_PKGS=(
                "base-devel"
                "pkgconf"
                "make"
                "gcc"
                "binutils"
            )
            DESKTOP_PKGS=(
                "desktop-file-utils"
                "gtk-update-icon-cache"
                "hicolor-icon-theme"
                "shared-mime-info"
                "xdg-utils"
                "glib2"
            )
            FONT_PKGS=(
                "fontconfig"
                "ttf-dejavu"
                "noto-fonts-emoji"
            )
            PYTHON_PKGS=(
                "python"
                "python-pip"
                "python-pillow"
                "librsvg"
                "optipng"
                "pngquant"
            )
            BENCH_PKGS=(
                "util-linux"
                "bc"
                "procps-ng"
            )
            PACKAGING_PKGS=(
                "zstd"
                "tar"
                "xz"
            )
            ;;

        zypper)
            CORE_PKGS=(
                "patterns-devel-base-devel_basis"
                "pkg-config"
                "make"
                "gcc"
                "binutils"
            )
            DESKTOP_PKGS=(
                "desktop-file-utils"
                "gtk3-tools"
                "hicolor-icon-theme"
                "shared-mime-info"
                "xdg-utils"
                "glib2-tools"
            )
            FONT_PKGS=(
                "fontconfig"
                "dejavu-fonts"
                "noto-coloremoji-fonts"
            )
            PYTHON_PKGS=(
                "python3"
                "python3-pip"
                "python3-Pillow"
                "rsvg-view"
                "optipng"
                "pngquant"
            )
            BENCH_PKGS=(
                "util-linux"
                "bc"
                "procps"
            )
            PACKAGING_PKGS=(
                "rpm-build"
                "zstd"
                "tar"
                "xz"
            )
            ;;

        apk)
            CORE_PKGS=("build-base" "pkgconf" "make" "binutils")
            DESKTOP_PKGS=("desktop-file-utils" "gtk-update-icon-cache" "hicolor-icon-theme" "shared-mime-info" "xdg-utils" "glib")
            FONT_PKGS=("fontconfig" "font-dejavu" "font-noto-emoji")
            PYTHON_PKGS=("python3" "py3-pip" "py3-pillow" "librsvg" "optipng")
            BENCH_PKGS=("util-linux" "bc" "procps")
            PACKAGING_PKGS=("zstd" "tar" "xz")
            ;;

        xbps)
            CORE_PKGS=("base-devel" "pkg-config" "make" "binutils")
            DESKTOP_PKGS=("desktop-file-utils" "gtk-update-icon-cache" "hicolor-icon-theme" "shared-mime-info" "xdg-utils" "glib")
            FONT_PKGS=("fontconfig" "dejavu-fonts" "noto-fonts-emoji")
            PYTHON_PKGS=("python3" "python3-pip" "python3-Pillow" "librsvg" "optipng")
            BENCH_PKGS=("util-linux" "bc" "procps-ng")
            PACKAGING_PKGS=("zstd" "tar" "xz")
            ;;

        *)
            CORE_PKGS=()
            DESKTOP_PKGS=()
            FONT_PKGS=()
            PYTHON_PKGS=()
            BENCH_PKGS=()
            PACKAGING_PKGS=()
            ;;
    esac
}

# Execute installation command with appropriate privileges
run_pkg_install() {
    local pkgs=("$@")
    if [[ ${#pkgs[@]} -eq 0 ]]; then
        return 0
    fi

    local cmd=()
    case "$PKG_MGR" in
        apt)
            cmd=("apt-get" "install" "-y" "${pkgs[@]}")
            ;;
        dnf)
            cmd=("dnf" "install" "-y" "${pkgs[@]}")
            ;;
        yum)
            cmd=("yum" "install" "-y" "${pkgs[@]}")
            ;;
        pacman)
            cmd=("pacman" "-S" "--noconfirm" "--needed" "${pkgs[@]}")
            ;;
        zypper)
            cmd=("zypper" "--non-interactive" "install" "${pkgs[@]}")
            ;;
        apk)
            cmd=("apk" "add" "${pkgs[@]}")
            ;;
        xbps)
            cmd=("xbps-install" "-y" "${pkgs[@]}")
            ;;
        *)
            error "Unsupported package manager '$PKG_MGR'."
            return 1
            ;;
    esac

    if [[ $DRY_RUN -eq 1 ]]; then
        info "[DRY-RUN] Would run: sudo ${cmd[*]}"
        return 0
    fi

    if [[ $(id -u) -eq 0 ]]; then
        info "Running package installation as root: ${cmd[*]}"
        "${cmd[@]}"
    elif command -v sudo &>/dev/null; then
        info "Running package installation with sudo: sudo ${cmd[*]}"
        sudo "${cmd[@]}"
    else
        error "Root or sudo privileges are required to install system packages."
        echo "Please run the following command as root:"
        echo "  ${cmd[*]}"
        return 1
    fi
}

# Check and setup Rust toolchain
setup_rust() {
    info "Checking Rust toolchain..."
    if command -v cargo &>/dev/null && command -v rustc &>/dev/null; then
        local rustc_ver
        rustc_ver="$(rustc --version 2>/dev/null || echo 'unknown')"
        local cargo_ver
        cargo_ver="$(cargo --version 2>/dev/null || echo 'unknown')"
        success "Rust toolchain is installed: ${rustc_ver} (${cargo_ver})"
    else
        warn "Rust toolchain (cargo/rustc) was not found in PATH."
        if [[ $CHECK_ONLY -eq 1 ]]; then
            warn "Rust toolchain is missing. Install via: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
            return 0
        fi

        if [[ $DRY_RUN -eq 1 ]]; then
            info "[DRY-RUN] Would install Rust via rustup (https://sh.rustup.rs)"
            return 0
        fi

        echo -e "${YELLOW}Would you like to install Rust via official rustup? [y/N]${NC} "
        if [[ $ASSUME_YES -eq 1 ]]; then
            REPLY="y"
        else
            read -r -n 1 -s REPLY || REPLY="n"
            echo
        fi

        if [[ "$REPLY" =~ ^[Yy]$ ]]; then
            info "Installing rustup toolchain..."
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            # shellcheck disable=SC1090
            source "$HOME/.cargo/env" 2>/dev/null || true
            success "Rust installed successfully: $(rustc --version)"
        else
            warn "Skipped Rust installation. Please install Rust manually before building Velox."
        fi
    fi

    # Ensure clippy and rustfmt components are available for `make lint`
    if command -v rustup &>/dev/null; then
        info "Verifying rustup components (clippy, rustfmt)..."
        if [[ $DRY_RUN -eq 1 ]]; then
            info "[DRY-RUN] Would run: rustup component add clippy rustfmt"
        elif [[ $CHECK_ONLY -eq 0 ]]; then
            rustup component add clippy rustfmt &>/dev/null || true
            success "Rustup components (clippy, rustfmt) verified."
        fi
    fi
}

# Main execution routine
main() {
    print_banner
    detect_distro

    info "Detected Operating System: ${BOLD}${DISTRO_ID}${NC} (Package Manager: ${BOLD}${PKG_MGR}${NC})"

    if [[ "$PKG_MGR" == "unknown" ]]; then
        warn "Could not automatically detect system package manager."
        warn "You may need to manually install dependencies for your distribution."
        setup_rust
        exit 0
    fi

    get_packages_for_manager

    local all_pkgs=("${CORE_PKGS[@]}" "${DESKTOP_PKGS[@]}" "${FONT_PKGS[@]}" "${PYTHON_PKGS[@]}" "${BENCH_PKGS[@]}" "${PACKAGING_PKGS[@]}")
    local missing_pkgs=()

    echo
    echo -e "${BOLD}Checking required system packages:${NC}"
    for pkg in "${all_pkgs[@]}"; do
        if is_pkg_installed "$pkg"; then
            echo -e "  [${GREEN}INSTALLED${NC}] $pkg"
        else
            echo -e "  [${YELLOW}MISSING${NC}]   $pkg"
            missing_pkgs+=("$pkg")
        fi
    done
    echo

    if [[ ${#missing_pkgs[@]} -eq 0 ]]; then
        success "All ${#all_pkgs[@]} system dependencies are already installed!"
    else
        warn "Found ${#missing_pkgs[@]} missing system package(s): ${missing_pkgs[*]}"

        if [[ $CHECK_ONLY -eq 1 ]]; then
            echo
            info "Run 'make bootstrap' or 'bash scripts/bootstrap_velox.sh -y' to install missing dependencies."
            setup_rust
            exit 0
        fi

        if [[ $ASSUME_YES -eq 0 && $DRY_RUN -eq 0 ]]; then
            echo -e "${YELLOW}Proceed with package installation? [Y/n]${NC} "
            read -r -n 1 -s REPLY || REPLY="y"
            echo
            if [[ "$REPLY" =~ ^[Nn]$ ]]; then
                warn "Installation aborted by user."
                exit 0
            fi
        fi

        # Update package list if apt
        if [[ "$PKG_MGR" == "apt" && $DRY_RUN -eq 0 ]]; then
            info "Updating apt package index..."
            if [[ $(id -u) -eq 0 ]]; then
                apt-get update -y -q
            elif command -v sudo &>/dev/null; then
                sudo apt-get update -y -q
            fi
        fi

        run_pkg_install "${missing_pkgs[@]}"
        success "System package installation completed."
    fi

    # Setup Rust toolchain
    echo
    setup_rust

    # Verify key dev tools
    echo
    info "Verifying development toolchain:"
    local tools=(
        "make"
        "gcc"
        "pkg-config"
        "python3"
        "gsettings"
        "xdg-open"
        "update-desktop-database"
        "gtk-update-icon-cache"
        "taskset"
        "dpkg-deb"
        "fakeroot"
        "zstd"
    )

    for tool in "${tools[@]}"; do
        if command -v "$tool" &>/dev/null; then
            echo -e "  [${GREEN}OK${NC}] $tool ($(command -v "$tool"))"
        else
            echo -e "  [${YELLOW}OPTIONAL/NOT FOUND${NC}] $tool"
        fi
    done

    # Python PIL verification
    if python3 -c "from PIL import Image" &>/dev/null; then
        echo -e "  [${GREEN}OK${NC}] Python Pillow (PIL) module is available"
    else
        echo -e "  [${YELLOW}WARN${NC}] Python Pillow (PIL) is not installed; icon generation may fall back to CLI tools"
    fi

    echo
    echo -e "${GREEN}==============================================================================${NC}"
    echo -e "${BOLD}${GREEN}   Velox Development Environment is Ready!${NC}"
    echo -e "${GREEN}==============================================================================${NC}"
    echo
    echo -e "Available commands via Makefile:"
    echo -e "  ${CYAN}make build${NC}              Build Velox in debug mode"
    echo -e "  ${CYAN}make release${NC}            Build Velox in optimized release mode (LTO, stripped)"
    echo -e "  ${CYAN}make test${NC}               Run full unit and integration test suite"
    echo -e "  ${CYAN}make lint${NC}               Run clippy (-D warnings) and code formatting check"
    echo -e "  ${CYAN}make generate-icons${NC}     Regenerate multi-resolution icon assets from SVG"
    echo -e "  ${CYAN}make bench${NC}              Run text render and throughput benchmark"
    echo -e "  ${CYAN}make deb${NC}                Build Debian/Ubuntu (.deb) distribution package"
    echo -e "  ${CYAN}make install${NC}            Install binary, desktop file, icons, and shell completions"
    echo
}

main "$@"
