# Third Party Dependencies

This directory contains vendored headers and libraries for external dependencies.

## Structure

- `minimap2/` - Minimap2 alignment headers (if needed)
- `zstd/` - Zstandard compression library headers
- `wgpu/` - WebGPU implementation headers (if needed for native builds)

## Purpose

Vendored dependencies ensure:
- Reproducible builds across platforms
- Version stability 
- Reduced external dependencies during compilation
- Support for air-gapped build environments