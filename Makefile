.PHONY: build lint update-version bench bench-grid bench-scrollback

VERSION_FILE := VERSION
VERSION := $(shell cat $(VERSION_FILE) | tr -d '\n\r')

# Build the project
build:
	cargo build

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

# Update the crate version in Cargo.toml from the VERSION file
update-version:
	@if [ -z "$(VERSION)" ]; then \
		echo "Error: VERSION file is empty"; \
		exit 1; \
	fi
	sed -i 's/^version = "[^"]*"/version = "$(VERSION)"/' Cargo.toml
	cargo check
