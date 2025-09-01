# DotX — Extreme-Scale Dot Plot

A revolutionary, offline, single-application dot-plot system that scales to ~100 Gb per axis with near 1 bp/px zoom, powered by a sparse multiresolution data model and optional GPU acceleration.

## Features

- **Extreme Scale**: Browse 10¹¹ bp axes with smooth interaction
- **Cross-Platform**: Windows, macOS, Linux support
- **GPU Accelerated**: Optional GPU processing for maximum performance
- **Deterministic**: Exact reproducible results with embedded provenance
- **Offline-First**: No internet required, no telemetry by default

## Architecture

- **Core**: Rust-based data structures and algorithms
- **UI**: egui + wgpu renderer for cross-platform graphics
- **Alignment**: Bundled minimap2 + custom GPU preview aligner
- **Storage**: Sparse multiresolution tile pyramid with LMDB indexing

## Installation

### Linux (Ubuntu/Debian)

#### Option 1: DEB Package (Recommended)
Download the latest `.deb` package from [Releases](https://github.com/machinelearning-guy/DotX/releases) and install:

```bash
sudo dpkg -i dotx_1.0.0_amd64.deb
sudo apt-get install -f  # if dependencies missing
```

#### Option 2: One-line installer
```bash
curl -sSL https://raw.githubusercontent.com/machinelearning-guy/DotX/main/scripts/install.sh | sudo bash
```

### Windows
Download the installer from [Releases](https://github.com/machinelearning-guy/DotX/releases) and run `DotX-Setup.msi`.

### macOS
Download the `.dmg` from [Releases](https://github.com/machinelearning-guy/DotX/releases) and install.

## Building from Source

```bash
cargo build --release
```

See [BUILDING.md](BUILDING.md) for detailed build instructions and cross-compilation.

## Usage

### GUI Application
```bash
cargo run --bin dotx-gui
```

### CLI
```bash
# Quick comparison
cargo run --bin dotx -- quick --ref ref.fa --qry qry.fa --preset mammal_hq --export fig.pdf

# Align and tile
cargo run --bin dotx -- align --ref ref.fa --qry qry.fa --preset mammal_hq --out run1.paf
cargo run --bin dotx -- tile --paf run1.paf --project proj.dotx --lod 0..10
cargo run --bin dotx -- plot --project proj.dotx --out fig.svg --view chr1:1-50M vs chr1:1-50M
```

## License

Apache-2.0