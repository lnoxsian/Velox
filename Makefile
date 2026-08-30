.PHONY: build release optimized-release lint update-version bench bench-grid bench-scrollback generate-icons

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

