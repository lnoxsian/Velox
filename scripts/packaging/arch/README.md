# Arch Linux Packaging for Velox

This directory contains the `PKGBUILD` and packaging script to produce Arch Linux packages (`.pkg.tar.zst`).

## Files in this Directory

- **`PKGBUILD`**: Standard Arch Linux packaging specification for building Velox from source. Compatible with `makepkg` and the Arch User Repository (AUR).
- **`build-arch.sh`**: Packaging script that builds an Arch package directly. If `makepkg` is installed on your machine, it invokes it; otherwise, it packages the compiled binary and assets into a valid `.pkg.tar.zst` archive using `tar` and `zstd`.

## Building the Package

### Option 1: Using `build-arch.sh` (Recommended)

```bash
# Package from repository root or scripts/packaging/arch/
./scripts/packaging/arch/build-arch.sh

# Or without rebuilding target binary:
./scripts/packaging/arch/build-arch.sh --no-build
```

### Option 2: Using `makepkg` (on Arch Linux)

```bash
cd scripts/packaging/arch
makepkg -sric
```

## Installing the Generated Package

```bash
sudo pacman -U dist/velox-*.pkg.tar.zst
```
