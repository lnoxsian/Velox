# Justfile for Velox

version := `cat VERSION | tr -d '\n\r'`

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

# Run the project in optimized release mode
run-optimized-release:
    cargo run --profile optimized-release

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

# Run only the grid benchmark test
bench-grid:
    bash benchmarks/text_render_test/testren.bash --grid

# Run only the scrollback benchmark test
bench-scrollback:
    bash benchmarks/text_render_test/testren.bash --scroll-back

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
