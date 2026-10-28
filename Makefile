PREFIX ?= /usr/local
DESTDIR ?=

# Allow lowercase prefix and destdir overrides for full compatibility with just syntax
ifneq ($(prefix),)
PREFIX := $(prefix)
endif
ifneq ($(destdir),)
DESTDIR := $(destdir)
endif

BINDIR ?= $(DESTDIR)$(PREFIX)/bin
DATADIR ?= $(DESTDIR)$(PREFIX)/share
APPDIR ?= $(DATADIR)/applications
ICONDIR ?= $(DATADIR)/icons/hicolor
BASHCOMPDIR ?= $(DATADIR)/bash-completion/completions
ZSHCOMPDIR ?= $(DATADIR)/zsh/site-functions
FISHCOMPDIR ?= $(DATADIR)/fish/vendor_completions.d
ICON_SIZES := 16x16 32x32 48x48 64x64 128x128 256x256 512x512 1024x1024

VERSION_FILE := VERSION
VERSION := $(shell cat $(VERSION_FILE) 2>/dev/null | tr -d '\n\r')

PROFILE ?= release
CORE ?= 0
DURATION ?=
WORKLOAD ?=
ARGS ?=

.PHONY: all build release optimized-release run run-release run-optimized-release check test clean update lint \
	update-version bump-version update-banner banner-update bench bench-grid bench-scrollback ram ram-usage ram-optimized-release ram-release \
	ram-debug cpu cpu-usage single-cpu test-single-cpu bench-cpu cpu-release cpu-optimized-release cpu-debug cpu-compare cpu-benchmark \
	palette generate-icons install install-desktop install-icons install-completions uninstall deb rpm arch-pkg list help

# Default target
all: build

# Generate multi-resolution icon assets
generate-icons:
	python3 scripts/generate_icons.py

# Build the project (debug mode)
build: generate-icons
	cargo build

# Build the project in release mode
release:
	cargo build --release

# Build the project in optimized release mode
optimized-release:
	cargo build --profile optimized-release

# Run the project (debug mode)
run:
	cargo run

# Run the project in release mode
run-release:
	cargo run --release

# Run the project in optimized release mode
run-optimized-release:
	cargo run --profile optimized-release

# Check the project for compilation errors
check:
	cargo check

# Run tests
test:
	cargo test

# Clean build artifacts
clean:
	cargo clean
	rm -rf dist

# Update dependencies in Cargo.lock
update:
	cargo update

# Run linters (clippy and formatter)
lint:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --all -- --check

# Install Velox desktop entry
install-desktop:
	install -d $(APPDIR)
	install -m 644 assets/io.github.lnoxsian.Velox.desktop $(APPDIR)/io.github.lnoxsian.Velox.desktop
	@if [ -z "$(DESTDIR)" ]; then \
		update-desktop-database -q $(APPDIR) 2>/dev/null || true; \
	fi

# Install Velox hicolor icons
install-icons:
	@for size in $(ICON_SIZES); do \
		install -d $(ICONDIR)/$$size/apps; \
		if [ -f "assets/generated_icons/icon_$$size.png" ]; then \
			install -m 644 assets/generated_icons/icon_$$size.png $(ICONDIR)/$$size/apps/io.github.lnoxsian.Velox.png; \
		fi; \
	done
	install -d $(ICONDIR)/scalable/apps
	if [ -f "assets/icons/velox_terminal_icon_final.svg" ]; then \
		install -m 644 assets/icons/velox_terminal_icon_final.svg $(ICONDIR)/scalable/apps/io.github.lnoxsian.Velox.svg; \
	fi
	@if [ -z "$(DESTDIR)" ]; then \
		gtk-update-icon-cache -q -t -f $(DATADIR)/icons/hicolor 2>/dev/null || true; \
	fi

# Install shell completions for Bash, Zsh, and Fish
install-completions:
	install -d $(BASHCOMPDIR)
	if [ -f "assets/completions/bash/velox" ]; then \
		install -m 644 assets/completions/bash/velox $(BASHCOMPDIR)/velox; \
	fi
	install -d $(ZSHCOMPDIR)
	if [ -f "assets/completions/zsh/_velox" ]; then \
		install -m 644 assets/completions/zsh/_velox $(ZSHCOMPDIR)/_velox; \
	fi
	install -d $(FISHCOMPDIR)
	if [ -f "assets/completions/fish/velox.fish" ]; then \
		install -m 644 assets/completions/fish/velox.fish $(FISHCOMPDIR)/velox.fish; \
	fi

# Install Velox binary, desktop entry, icons, and shell completions
install: release install-desktop install-icons install-completions
	install -d $(BINDIR)
	install -m 755 target/release/velox $(BINDIR)/velox

# Uninstall Velox binary, desktop entry, icons, and shell completions
uninstall:
	rm -f $(BINDIR)/velox
	rm -f $(APPDIR)/io.github.lnoxsian.Velox.desktop
	@for size in $(ICON_SIZES); do \
		rm -f $(ICONDIR)/$$size/apps/io.github.lnoxsian.Velox.png; \
	done
	rm -f $(ICONDIR)/scalable/apps/io.github.lnoxsian.Velox.svg
	rm -f $(BASHCOMPDIR)/velox
	rm -f $(ZSHCOMPDIR)/_velox
	rm -f $(FISHCOMPDIR)/velox.fish
	@if [ -z "$(DESTDIR)" ]; then \
		echo "Updating icon and desktop caches..."; \
		gtk-update-icon-cache -q -t -f $(DATADIR)/icons/hicolor 2>/dev/null || true; \
		update-desktop-database -q $(APPDIR) 2>/dev/null || true; \
	fi

# Run the benchmark test script
bench:
	bash scripts/text_render_test/testren.bash

bench-grid:
	bash scripts/text_render_test/testren.bash --grid

bench-scrollback:
	bash scripts/text_render_test/testren.bash --scroll-back

# Measure RAM usage with selectable profile (default: release, or PROFILE=optimized-release / PROFILE=debug)
ram-usage:
	python3 scripts/measure_ram.py --profile $(PROFILE) $(ARGS)

# Alias for measuring RAM usage
ram: ram-usage

# Measure RAM usage in optimized-release mode
ram-optimized-release:
	python3 scripts/measure_ram.py --profile optimized-release $(ARGS)

# Measure RAM usage in release mode
ram-release:
	python3 scripts/measure_ram.py --profile release $(ARGS)

# Measure RAM usage in debug mode
ram-debug:
	python3 scripts/measure_ram.py --profile debug $(ARGS)

# Measure and profile single-core CPU usage (default: release, or PROFILE=optimized-release / PROFILE=debug, CORE=0)
cpu-usage:
	python3 scripts/measure_cpu.py --profile $(PROFILE) --core $(CORE) $(ARGS)

# Aliases for single-core CPU usage profiling and testing
cpu: cpu-usage
single-cpu: cpu-usage
bench-cpu: cpu-usage

# Test terminal throughput and CPU efficiency on a single CPU core (5-second workload by default)
test-single-cpu:
	python3 scripts/measure_cpu.py --profile $(PROFILE) --core $(CORE) --workload $(if $(WORKLOAD),$(WORKLOAD),stream-ascii) $(if $(DURATION),--duration $(DURATION),--duration 5) $(ARGS)

# Run standardized single-core CPU benchmark workload (WORKLOAD=stream-ascii/stream-ansi/scroll/idle, DURATION=5)
cpu-benchmark: test-single-cpu

# Measure single-core CPU usage in release mode
cpu-release:
	python3 scripts/measure_cpu.py --profile release --core $(CORE) $(ARGS)

# Measure single-core CPU usage in optimized-release mode
cpu-optimized-release:
	python3 scripts/measure_cpu.py --profile optimized-release --core $(CORE) $(ARGS)

# Measure single-core CPU usage in debug mode
cpu-debug:
	python3 scripts/measure_cpu.py --profile debug --core $(CORE) $(ARGS)

# Benchmark and compare single-core CPU performance across all build profiles (debug, release, optimized-release)
cpu-compare:
	python3 scripts/measure_cpu.py --all-profiles --core $(CORE) $(ARGS)

# Display ANSI 256 / 24-bit color test
palette:
	bash scripts/colortest

# Update the crate version in Cargo.toml and README.md from the VERSION file
update-version:
	@if [ -z "$(VERSION)" ]; then \
		echo "Error: VERSION file is empty"; \
		exit 1; \
	fi
	sed -i 's/^version = "[^"]*"/version = "$(VERSION)"/' Cargo.toml
	@if [ -f "README.md" ]; then \
		sed -i -E 's/version-v[0-9]+\.[0-9]+\.[0-9]+/version-v$(VERSION)/g' README.md; \
		sed -i -E 's/alt="Version [0-9]+\.[0-9]+\.[0-9]+"/alt="Version $(VERSION)"/g' README.md; \
		sed -i -E 's/\*\*Velox v[0-9]+\.[0-9]+\.[0-9]+\*\*/\*\*Velox v$(VERSION)\*\*/g' README.md; \
		sed -i -E 's/\|\s*\*\*Version\*\*\s*\|\s*`v[0-9]+\.[0-9]+\.[0-9]+`\s*\|/| **Version** | `v$(VERSION)` |/g' README.md; \
	fi
	@echo "Synchronized version $(VERSION) to Cargo.toml and README.md."
	cargo check

# Interactively prompt and update version across VERSION, Cargo.toml, and README.md
bump-version:
	bash scripts/update-version.sh
	cargo check

# Update ASCII banner in src/cli.rs from assets/ascii_banner.txt
update-banner:
	bash scripts/banner_update.sh
	cargo check

# Alias banner-update to update-banner
banner-update: update-banner


# Package as Debian / Ubuntu (.deb) package
deb:
	bash scripts/packaging/debian/build-deb.sh

# Package for Arch Linux (.pkg.tar.zst)
arch-pkg:
	bash scripts/packaging/arch/build-arch.sh

# Package as Fedora / RHEL (.rpm) package
rpm:
	bash scripts/packaging/fedora/build-rpm.sh

# List all commands and available targets with descriptions
list:
	@printf "\n\033[1;36mVelox v$(VERSION) - Available Makefile Targets\033[0m\n\n"
	@printf "\033[1mUsage:\033[0m make \033[36m<target>\033[0m [\033[33mVARIABLE=value\033[0m...]\n\n"
	@printf "\033[1;32mBuilding & Compilation:\033[0m\n"
	@printf "  \033[36m%-25s\033[0m %s\n" "build" "Build binary in debug mode (generates icons first)"
	@printf "  \033[36m%-25s\033[0m %s\n" "release" "Build binary in release mode"
	@printf "  \033[36m%-25s\033[0m %s\n" "optimized-release" "Build binary with fat LTO, codegen-units=1, stripped"
	@printf "  \033[36m%-25s\033[0m %s\n" "generate-icons" "Generate multi-resolution icon assets"
	@printf "  \033[36m%-25s\033[0m %s\n" "clean" "Remove target/ and dist/ artifacts"
	@printf "\n\033[1;32mRunning:\033[0m\n"
	@printf "  \033[36m%-25s\033[0m %s\n" "run" "Run Velox in debug mode"
	@printf "  \033[36m%-25s\033[0m %s\n" "run-release" "Run Velox in release mode"
	@printf "  \033[36m%-25s\033[0m %s\n" "run-optimized-release" "Run Velox in optimized release mode"
	@printf "\n\033[1;32mTesting & Quality:\033[0m\n"
	@printf "  \033[36m%-25s\033[0m %s\n" "check" "Type-check code without compiling binary"
	@printf "  \033[36m%-25s\033[0m %s\n" "test" "Run full test suite (unit + integration tests)"
	@printf "  \033[36m%-25s\033[0m %s\n" "lint" "Run clippy (-D warnings) and cargo fmt checks"
	@printf "  \033[36m%-25s\033[0m %s\n" "palette" "Display ANSI 256 and truecolor terminal test"
	@printf "\n\033[1;32mPackaging & Distribution:\033[0m\n"
	@printf "  \033[36m%-25s\033[0m %s\n" "deb" "Build Debian / Ubuntu package in dist/ (*.deb)"
	@printf "  \033[36m%-25s\033[0m %s\n" "arch-pkg" "Build Arch Linux package in dist/ (*.pkg.tar.zst)"
	@printf "  \033[36m%-25s\033[0m %s\n" "rpm" "Build Fedora / RHEL RPM package in dist/ (*.rpm)"
	@printf "\n\033[1;32mInstallation & Desktop:\033[0m\n"
	@printf "  \033[36m%-25s\033[0m %s\n" "install" "Install binary, desktop file, icons, and completions (PREFIX=/usr/local)"
	@printf "  \033[36m%-25s\033[0m %s\n" "install-desktop" "Install .desktop launcher entry"
	@printf "  \033[36m%-25s\033[0m %s\n" "install-icons" "Install hicolor icons for all supported resolutions"
	@printf "  \033[36m%-25s\033[0m %s\n" "install-completions" "Install Bash, Zsh, and Fish shell completions"
	@printf "  \033[36m%-25s\033[0m %s\n" "uninstall" "Remove installed binary, desktop file, icons, and completions"
	@printf "\n\033[1;32mBenchmarking & Performance:\033[0m\n"
	@printf "  \033[36m%-25s\033[0m %s\n" "bench" "Run text rendering benchmark script"
	@printf "  \033[36m%-25s\033[0m %s\n" "bench-grid" "Run text rendering benchmark in grid mode"
	@printf "  \033[36m%-25s\033[0m %s\n" "bench-scrollback" "Run text rendering benchmark in scrollback mode"
	@printf "  \033[36m%-25s\033[0m %s\n" "cpu / cpu-usage" "Profile terminal pinned to single CPU core (CORE=0, PROFILE=...)"
	@printf "  \033[36m%-25s\033[0m %s\n" "test-single-cpu" "Run single-core CPU throughput test (WORKLOAD=..., DURATION=...)"
	@printf "  \033[36m%-25s\033[0m %s\n" "cpu-benchmark" "Alias for test-single-cpu standardized workload"
	@printf "  \033[36m%-25s\033[0m %s\n" "cpu-release" "Measure single-core CPU usage of release build"
	@printf "  \033[36m%-25s\033[0m %s\n" "cpu-optimized-release" "Measure single-core CPU usage of optimized release build"
	@printf "  \033[36m%-25s\033[0m %s\n" "cpu-debug" "Measure single-core CPU usage of debug build"
	@printf "  \033[36m%-25s\033[0m %s\n" "cpu-compare" "Compare single-core CPU usage across all profiles"
	@printf "  \033[36m%-25s\033[0m %s\n" "ram / ram-usage" "Measure RAM usage (optional: PROFILE=..., ARGS=...)"
	@printf "  \033[36m%-25s\033[0m %s\n" "ram-release" "Measure RAM usage of release build"
	@printf "  \033[36m%-25s\033[0m %s\n" "ram-optimized-release" "Measure RAM usage of optimized release build"
	@printf "  \033[36m%-25s\033[0m %s\n" "ram-debug" "Measure RAM usage of debug build"
	@printf "\n\033[1;32mVersion Management:\033[0m\n"
	@printf "  \033[36m%-25s\033[0m %s\n" "update" "Update Cargo dependencies in Cargo.lock"
	@printf "  \033[36m%-25s\033[0m %s\n" "update-version" "Sync VERSION file to Cargo.toml and README.md"
	@printf "  \033[36m%-25s\033[0m %s\n" "bump-version" "Interactive prompt to bump version across files"
	@printf "  \033[36m%-25s\033[0m %s\n" "update-banner" "Update ASCII banner in CLI from assets/ascii_banner.txt"
	@printf "  \033[36m%-25s\033[0m %s\n" "list / help" "Show this command list and help guide"
	@printf "\n\033[1mVariables:\033[0m\n"
	@printf "  \033[33mPREFIX\033[0m       Installation prefix (default: /usr/local)\n"
	@printf "  \033[33mDESTDIR\033[0m      Staging root directory for packaging\n"
	@printf "  \033[33mPROFILE\033[0m      Cargo profile (default: release or optimized-release)\n"
	@printf "  \033[33mCORE\033[0m         CPU core ID to pin single-core tests (default: 0)\n"
	@printf "  \033[33mWORKLOAD\033[0m     Workload for CPU tests (stream-ascii, stream-ansi, scroll, idle)\n"
	@printf "  \033[33mDURATION\033[0m     Duration in seconds for CPU test workloads (default: 5)\n"
	@printf "  \033[33mARGS\033[0m         Extra command arguments passed to scripts\n\n"

# Alias help to list
help: list


