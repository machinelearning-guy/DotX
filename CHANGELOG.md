# Changelog

All notable changes to DotX will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Linux DEB package with full desktop integration
- Windows MSI installer with setup wizard
- Cross-platform build system with GitHub Actions
- Comprehensive file dialog implementation using rfd
- Multiple bioinformatics alignment presets (fungal, metagenomic, plant_genome, prokaryotic, repetitive)
- FASTA file format validation with detailed error reporting
- Robust PAF file parsing with coordinate validation
- Professional desktop integration (icons, MIME types, file associations)
- One-line Linux installer script
- Comprehensive build documentation

### Fixed
- Memory safety issues in FASTA file processing
- Integer overflow protection in coordinate calculations
- Proper error handling replacing panic-prone `.unwrap()` calls
- Bounds checking for array/slice access
- Median calculation edge cases in alignment statistics
- Coordinate validation in genome transformations

### Changed
- Enhanced GUI with working file dialogs and validation
- Improved CLI with expanded preset options
- Better error messages and user feedback
- Optimized release builds with proper compression

### Security
- Removed unsafe memory operations
- Added input validation for all file formats
- Proper handling of malformed input files

## [1.0.0] - Initial Release

### Added
- Core dot plot visualization engine
- Cross-platform GUI using egui/eframe
- Command-line interface with alignment and plotting commands
- Support for FASTA and PAF file formats
- GPU acceleration support via wgpu
- Sparse multiresolution data model
- Basic alignment presets (bacterial, plant_te, mammal, viral)
- Project file format (.dotx)
- Memory-mapped file I/O for large datasets
- Minimap2 integration for sequence alignment