# Fedora / RHEL RPM Packaging for Velox

This directory contains the RPM specification (`velox.spec`) and packaging script to produce Red Hat / Fedora / CentOS binary RPM packages (`.rpm`).

## Files in this Directory

- **`velox.spec`**: Standard RPM spec file complying with Fedora packaging guidelines.
- **`build-rpm.sh`**: Packaging script that configures an RPM build tree, stages the source archive, and runs `rpmbuild` to generate the `.rpm` packages into this directory.

## Building the Package

On Fedora, RHEL, or CentOS systems with `rpm-build` installed:

```bash
# Install build prerequisites if needed:
sudo dnf install -y rpm-build rpmdevtools cargo rust

# Build the RPM package from repository root:
./scripts/packaging/fedora/build-rpm.sh

# Or without re-compiling existing binary:
./scripts/packaging/fedora/build-rpm.sh --no-build
```

## Installing the Generated Package

```bash
sudo dnf install dist/velox-*.rpm
```
