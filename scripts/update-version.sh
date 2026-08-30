#!/usr/bin/env bash
set -euo pipefail

# Paths to the version-related files
VERSION_FILE="VERSION"
CARGO_TOML="Cargo.toml"

# Read the current version
if [ -f "$VERSION_FILE" ]; then
    CURRENT_VERSION=$(tr -d '\n\r' < "$VERSION_FILE")
else
    CURRENT_VERSION=$(grep -m 1 '^version = ' "$CARGO_TOML" | cut -d '"' -f 2)
fi

# Prompt the user for the new version
# Use /dev/tty for input if stdin is not a tty (e.g. running from Justfile/non-interactive shell)
if [ -t 0 ]; then
    read -p "update version [$CURRENT_VERSION] : " NEW_VERSION
else
    read -p "update version [$CURRENT_VERSION] : " NEW_VERSION < /dev/tty
fi

# If no version was entered, keep the current one and exit
if [ -z "$NEW_VERSION" ]; then
    echo "No version entered. Keeping version $CURRENT_VERSION."
    exit 0
fi

# Update VERSION file
echo "$NEW_VERSION" > "$VERSION_FILE"

# Update Cargo.toml version field
sed -i 's/^version = "[^"]*"/version = "'"$NEW_VERSION"'"/' "$CARGO_TOML"

# Update README.md version badges and mentions from VERSION
if [ -f "README.md" ]; then
    sed -i -E "s/version-v[0-9]+\.[0-9]+\.[0-9]+/version-v$NEW_VERSION/g" README.md
    sed -i -E "s/alt=\"Version [0-9]+\.[0-9]+\.[0-9]+\"/alt=\"Version $NEW_VERSION\"/g" README.md
    sed -i -E "s/\*\*Velox v[0-9]+\.[0-9]+\.[0-9]+\*\*/\*\*Velox v$NEW_VERSION\*\*/g" README.md
    sed -i -E "s/\|\s*\*\*Version\*\*\s*\|\s*`v[0-9]+\.[0-9]+\.[0-9]+`\s*\|/| **Version** | \`v$NEW_VERSION\` |/g" README.md
fi

echo "Version updated from $CURRENT_VERSION to $NEW_VERSION across VERSION, Cargo.toml, and README.md."

