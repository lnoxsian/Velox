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

# Measure RAM usage with selectable profile (default: release, or optimized-release / debug)
ram-usage profile="release" *args="":
    python3 scripts/measure_ram.py --profile {{profile}} {{args}}

# Alias for measuring RAM usage
ram profile="release" *args="":
    python3 scripts/measure_ram.py --profile {{profile}} {{args}}

# Measure RAM usage in optimized-release mode
ram-optimized-release *args="":
    python3 scripts/measure_ram.py --profile optimized-release {{args}}

# Measure RAM usage in release mode
ram-release *args="":
    python3 scripts/measure_ram.py --profile release {{args}}

# Measure RAM usage in debug mode
ram-debug *args="":
    python3 scripts/measure_ram.py --profile debug {{args}}

# Run linters (clippy and formatter)
lint:
    cargo clippy --all-targets -- -D warnings
    cargo fmt --all -- --check

# Update the crate version and README.md from the VERSION file
update-version:
    @if [ -z "{{version}}" ]; then echo "Error: VERSION file is empty"; exit 1; fi
    sed -i 's/^version = "[^"]*"/version = "{{version}}"/' Cargo.toml
    @if [ -f "README.md" ]; then \
        sed -i -E 's/version-v[0-9]+\.[0-9]+\.[0-9]+/version-v{{version}}/g' README.md; \
        sed -i -E 's/alt="Version [0-9]+\.[0-9]+\.[0-9]+"/alt="Version {{version}}"/g' README.md; \
        sed -i -E 's/\*\*Velox v[0-9]+\.[0-9]+\.[0-9]+\*\*/\*\*Velox v{{version}}\*\*/g' README.md; \
        sed -i -E 's/\|\s*\*\*Version\*\*\s*\|\s*`v[0-9]+\.[0-9]+\.[0-9]+`\s*\|/| **Version** | `v{{version}}` |/g' README.md; \
    fi
    @echo "Synchronized version {{version}} to Cargo.toml and README.md."
    cargo check

# Interactively prompt and update version across VERSION, Cargo.toml, and README.md
bump-version:
    bash scripts/update-version.sh
    cargo check

# Update dependencies in Cargo.lock
update:
    cargo update

# Alias for colors
palette:
    bash scripts/colortest

# Clean build artifacts
clean:
    cargo clean
