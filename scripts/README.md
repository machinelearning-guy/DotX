# Build and Release Scripts

This directory contains scripts for building, testing, and releasing DOTx.

## Structure

- `build/` - Build automation scripts
  - `build-all.sh` - Cross-platform build script
  - `build-release.sh` - Release build with optimizations
  - `setup-dev.sh` - Development environment setup
- `release/` - Release management scripts
  - `package.sh` - Create distribution packages
  - `sign-binaries.sh` - Code signing for releases
  - `upload-release.sh` - Upload to release channels
- `ci/` - Continuous integration scripts
  - `test-all.sh` - Comprehensive test runner
  - `benchmark.sh` - CI benchmark runner
  - `security-scan.sh` - Security vulnerability scanning
- `util/` - Utility scripts
  - `clean.sh` - Clean build artifacts
  - `format.sh` - Code formatting
  - `lint.sh` - Code linting

## Usage

Most scripts are invoked via the Makefile:

```bash
# Use Makefile targets (recommended)
make build
make test
make dist

# Direct script usage
./scripts/build/build-all.sh
./scripts/ci/test-all.sh
```

## Requirements

Scripts may require:
- Bash 4.0+
- Cross-compilation toolchains
- Code signing certificates (for releases)
- Platform-specific tools (Docker, etc.)