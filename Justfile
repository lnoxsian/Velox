# Justfile for Velox

version := `cat VERSION | tr -d '\n\r'`

# Build the project
build:
    cargo build

# Run linters (clippy and formatter)
lint:
    cargo clippy --all-targets -- -D warnings
    cargo fmt --all -- --check

# Update the crate version in Cargo.toml from the VERSION file
update-version:
    sed -i 's/^version = "[^"]*"/version = "{{version}}"/' Cargo.toml
    cargo check
