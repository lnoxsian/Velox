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

# Update the crate version using update-version script
update-version:
    bash scripts/update-version.sh
    cargo check

# Update dependencies in Cargo.lock
update:
    cargo update

# Display terminal color palette (ANSI, 256-color, Truecolor)
colors:
    bash scripts/colors.sh

# Alias for colors
palette:
    bash scripts/colors.sh

# Clean build artifacts
clean:
    cargo clean
