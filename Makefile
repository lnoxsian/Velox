.PHONY: build lint update-version

VERSION_FILE := VERSION
VERSION := $(shell cat $(VERSION_FILE) | tr -d '\n\r')

# Build the project
build:
	cargo build

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
