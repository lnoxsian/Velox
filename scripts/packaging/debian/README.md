# Debian / Ubuntu Packaging for Velox

This directory contains the packaging script and artifacts to build a Debian binary package (`.deb`) for Velox.

## Building the .deb Package

To build the Debian package using the optimized release binary:

```bash
# From repository root or scripts/packaging/debian/
./scripts/packaging/debian/build-deb.sh
```

### Options

```bash
./scripts/packaging/debian/build-deb.sh [OPTIONS]

Options:
    --profile <PROFILE>     Cargo build profile ('optimized-release' or 'release', default: optimized-release)
    --no-build              Skip compiling; package existing target binary immediately
    --output-dir <DIR>      Output directory for .deb file (default: dist/)
    -h, --help              Show help message
```

## Installing the Generated Package

```bash
sudo dpkg -i dist/velox_*.deb

# If any dependencies are missing:
sudo apt-get install -f
```

## Package Contents

- **Executable:** `/usr/bin/velox`
- **Desktop Entry:** `/usr/share/applications/io.github.lnoxsian.Velox.desktop`
- **Application Icons:** `/usr/share/icons/hicolor/<size>/apps/io.github.lnoxsian.Velox.png` (16x16 up to 1024x1024) and scalable SVG
- **Documentation & Copyright:** `/usr/share/doc/velox/README.md` and `/usr/share/doc/velox/copyright`
- **Post-Install Hooks:** Automatically refreshes desktop database and GTK icon cache
