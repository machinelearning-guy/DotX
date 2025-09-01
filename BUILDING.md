# Building DotX

## Prerequisites

### All Platforms
- Rust 1.70+ 
- Git

### Linux
- pkg-config
- libgtk-3-dev (for GUI)
- libssl-dev

### Windows  
- Visual Studio Build Tools or Visual Studio Community
- Windows SDK

### macOS
- Xcode Command Line Tools

## Building

### Standard Build
```bash
cargo build --release
```

### Cross-compilation for Windows (from Linux)
```bash
# Install cross-compilation target
rustup target add x86_64-pc-windows-gnu

# Install mingw-w64 toolchain
sudo apt-get install gcc-mingw-w64-x86-64

# Build for Windows
cargo build --release --target x86_64-pc-windows-gnu
```

### Windows Build Script
On Windows, use the PowerShell script:
```powershell
.\scripts\build-windows.ps1 -Release -Installer
```

## Creating Windows Installer

### Prerequisites
1. Install WiX Toolset v3: https://wixtoolset.org/releases/
2. Ensure WiX tools are in PATH

### Build Steps
1. Build binaries: `.\scripts\build-windows.ps1 -Release`
2. Create installer: `cd installer && .\build-installer.bat`

## Distribution

### Automated Releases
GitHub Actions automatically builds releases for:
- Windows (x64)
- Linux (x64) 
- macOS (x64 and ARM64)

### Manual Release
```bash
# Tag a release
git tag v1.0.0
git push --tags

# This triggers the build-release workflow
```

## Development

### Running Tests
```bash
cargo test
```

### Running with Logging
```bash
RUST_LOG=debug cargo run --bin dotx-gui
```

### Cross-platform Testing
Use the provided GitHub Actions workflow in `.github/workflows/build-release.yml`