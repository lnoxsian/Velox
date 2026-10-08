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
ICON_SIZES := 16x16 32x32 48x48 64x64 128x128 256x256 512x512 1024x1024

VERSION_FILE := VERSION
VERSION := $(shell cat $(VERSION_FILE) 2>/dev/null | tr -d '\n\r')

PROFILE ?= release
ARGS ?=

.PHONY: all build release optimized-release run run-release run-optimized-release check test clean update lint \
	update-version bump-version bench bench-grid bench-scrollback ram ram-usage ram-optimized-release ram-release \
	ram-debug palette generate-icons install install-desktop install-icons uninstall

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

# Install Velox binary, desktop entry, and icons
install: release install-desktop install-icons
	install -d $(BINDIR)
	install -m 755 target/release/velox $(BINDIR)/velox

# Uninstall Velox binary, desktop entry, and icons
uninstall:
	rm -f $(BINDIR)/velox
	rm -f $(APPDIR)/io.github.lnoxsian.Velox.desktop
	@for size in $(ICON_SIZES); do \
		rm -f $(ICONDIR)/$$size/apps/io.github.lnoxsian.Velox.png; \
	done
	rm -f $(ICONDIR)/scalable/apps/io.github.lnoxsian.Velox.svg
	@if [ -z "$(DESTDIR)" ]; then \
		echo "Updating icon and desktop caches..."; \
		gtk-update-icon-cache -q -t -f $(DATADIR)/icons/hicolor 2>/dev/null || true; \
		update-desktop-database -q $(APPDIR) 2>/dev/null || true; \
	fi

# Run the benchmark test script
bench:
	bash benchmarks/text_render_test/testren.bash

bench-grid:
	bash benchmarks/text_render_test/testren.bash --grid

bench-scrollback:
	bash benchmarks/text_render_test/testren.bash --scroll-back

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
