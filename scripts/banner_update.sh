#!/usr/bin/env bash
set -euo pipefail

# Determine repository root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BANNER_FILE="${1:-$REPO_ROOT/assets/ascii_banner.txt}"
CLI_FILE="${2:-$REPO_ROOT/src/cli.rs}"

if [ ! -f "$BANNER_FILE" ]; then
    echo "Error: Banner file not found: $BANNER_FILE" >&2
    exit 1
fi

if [ ! -f "$CLI_FILE" ]; then
    echo "Error: CLI file not found: $CLI_FILE" >&2
    exit 1
fi

python3 - << 'EOF' "$BANNER_FILE" "$CLI_FILE"
import sys
import re

banner_path = sys.argv[1]
cli_path = sys.argv[2]

with open(banner_path, "r", encoding="utf-8") as f:
    banner = f.read().rstrip("\r\n")

# Escape backslashes and double quotes for Rust string literal
escaped_banner = banner.replace("\\", "\\\\").replace('"', '\\"')

with open(cli_path, "r", encoding="utf-8") as f:
    cli_content = f.read()

# Match the print_banner function in src/cli.rs
pattern = r'(pub fn print_banner\(\)\s*\{\s*println!\(\s*")([\s\S]*?)("\s*\);)'

if not re.search(pattern, cli_content):
    print(f"Error: Could not locate banner pattern in {cli_path}", file=sys.stderr)
    sys.exit(1)

new_cli_content = re.sub(
    pattern,
    lambda m: f"{m.group(1)}{escaped_banner}{m.group(3)}",
    cli_content,
    count=1
)

with open(cli_path, "w", encoding="utf-8") as f:
    f.write(new_cli_content)

print(f"Successfully updated banner in {cli_path} from {banner_path}")
EOF
