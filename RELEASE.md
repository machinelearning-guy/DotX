# DotX Release Process

## Pre-Release Checklist

### 1. Code Quality
- [ ] All tests pass: `cargo test`
- [ ] No compiler warnings: `cargo clippy -- -W clippy::all`
- [ ] Code formatted: `cargo fmt --check`
- [ ] Documentation updated

### 2. Version Management
- [ ] Update version in `Cargo.toml` files
- [ ] Update version in `packaging/debian/control`
- [ ] Update version in installer configs
- [ ] Create git tag: `git tag v1.0.0`

### 3. Platform Builds
- [ ] Linux build tested: `cargo build --release`
- [ ] DEB package created: `./scripts/build-deb.sh`
- [ ] Windows cross-compilation tested
- [ ] macOS builds (if available)

### 4. Documentation
- [ ] README.md updated with installation instructions
- [ ] BUILDING.md reflects current build process
- [ ] CHANGELOG.md updated with new features and fixes
- [ ] License files present and correct

## Release Process

### 1. Automated Release (Recommended)
Push a version tag to trigger automated builds:

```bash
git tag v1.0.0
git push origin v1.0.0
```

This triggers GitHub Actions to:
- Build for all platforms (Linux, Windows, macOS)
- Create DEB packages for Linux
- Generate release archives
- Create GitHub Release with artifacts

### 2. Manual Release
If needed, build manually:

```bash
# Linux DEB package
./scripts/build-deb.sh 1.0.0

# Windows (if cross-compiling from Linux)
cargo build --release --target x86_64-pc-windows-gnu

# Create archives
tar -czf dotx-linux-x64.tar.gz -C target/release dotx dotx-gui README.md
```

## Post-Release

### 1. Verification
- [ ] Download and test packages on clean systems
- [ ] Verify desktop integration works
- [ ] Test CLI and GUI functionality
- [ ] Check that uninstaller works

### 2. Distribution
- [ ] Update package repositories (if applicable)
- [ ] Notify users through appropriate channels
- [ ] Update documentation sites
- [ ] Social media announcement (if applicable)

## Release Artifacts

Each release should include:

### Linux
- `dotx-linux-x64.tar.gz` - Binary archive
- `dotx_1.0.0_amd64.deb` - Debian package

### Windows  
- `dotx-windows-x64.zip` - Binary archive
- `DotX-Setup.msi` - Installer

### macOS
- `dotx-macos-x64.tar.gz` - Intel binary archive  
- `dotx-macos-arm64.tar.gz` - Apple Silicon binary archive

## Version Numbering

Follow Semantic Versioning (semver):
- `MAJOR.MINOR.PATCH` (e.g., 1.2.3)
- Major: Breaking changes
- Minor: New features, backward compatible
- Patch: Bug fixes, backward compatible

## Rollback Plan

If issues are discovered:
1. Mark release as pre-release in GitHub
2. Fix issues in hotfix branch
3. Create new patch release
4. Deprecate problematic release