PREFIX ?= /usr/local
BINDIR ?= $(DESTDIR)$(PREFIX)/bin
DATADIR ?= $(DESTDIR)$(PREFIX)/share
APPDIR ?= $(DATADIR)/applications
ICONDIR ?= $(DATADIR)/icons/hicolor
ICON_SIZES := 16x16 32x32 48x48 64x64 128x128 256x256 512x512 1024x1024

.PHONY: build release optimized-release lint update-version bench bench-grid bench-scrollback generate-icons install install-desktop install-icons uninstall

VERSION_FILE := VERSION
VERSION := $(shell cat $(VERSION_FILE) | tr -d '\n\r')

# Generate multi-resolution icon assets
generate-icons:
	python3 scripts/generate_icons.py

# Build the project
build: generate-icons
	cargo build

# Build the project in release mode
release:
	cargo build --release

# Build the project in optimized release mode
optimized-release:
	cargo build --profile optimized-release

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

# Run linters (clippy and formatter)
lint:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --all -- --check

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
