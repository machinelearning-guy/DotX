# DOTx Documentation

This directory contains comprehensive documentation for the DOTx genome visualization toolkit.

## Structure

- `user-guide/` - End-user documentation
  - `installation.md` - Installation instructions
  - `quick-start.md` - Getting started guide
  - `cli-reference.md` - Command-line interface documentation
  - `gui-tutorial.md` - Desktop application tutorial
- `developer/` - Developer documentation  
  - `architecture.md` - System architecture overview
  - `building.md` - Building from source
  - `contributing.md` - Contribution guidelines
  - `api/` - API documentation
- `algorithms/` - Technical documentation
  - `dot-plot-generation.md` - Dot plot algorithm details
  - `gpu-acceleration.md` - GPU compute implementation
  - `file-formats.md` - Supported file formats
- `examples/` - Usage examples and tutorials

## Building Documentation

```bash
# Generate API docs
make docs

# Build user guide (if using mdBook)
mdbook build docs/user-guide

# Serve documentation locally
mdbook serve docs/user-guide
```

## Contributing

Documentation contributions are welcome! Please ensure:
- Clear, concise writing
- Code examples are tested
- Screenshots are up-to-date
- Cross-references are maintained