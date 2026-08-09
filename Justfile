# Justfile for Velox

version := `cat VERSION | tr -d '\n\r'`

# Build the project
build:
    cargo build

# Build the project in release mode
release:
    cargo build --release

# Check the project for compilation errors
check:
    cargo check

# Run the project
run:
    cargo run

# Run the project in release mode
run-release:
    cargo run --release

# Run the tests
test:
    cargo test

# Run the benchmarks
bench:
    bash benchmarks/text_render_test/testren.bash

# Run linters (clippy and formatter)
lint:
    cargo clippy --all-targets -- -D warnings
    cargo fmt --all -- --check

# Update the crate version using update-version script
update-version:
    bash scripts/update-version.sh
    cargo check

# Update dependencies in Cargo.lock
update:
    cargo update

# Clean build artifacts
clean:
    cargo clean
