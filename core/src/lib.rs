// DOTx Core Library
// Core algorithms and data structures for genome analysis and dot plot generation

#![warn(missing_docs)]

//! DOTx Core
//! 
//! This crate provides the core functionality for genome sequence analysis,
//! dot plot generation, and high-performance data processing.

/// Core data types and structures
pub mod types;

/// Sequence alignment algorithms  
pub mod alignment;

/// Genome file parsing (FASTA, etc.)
pub mod fasta;

/// Tiling and hierarchical data structures
pub mod tiles;

/// Coordinates and genomic ranges
pub mod coords;

/// File I/O parsers for bioinformatics formats
pub mod io;

/// DOTx binary database format
pub mod dotxdb;

/// Serialization and compression utilities
pub mod serialization;

// Re-export commonly used types
pub use types::*;
pub use dotxdb::{
    DotxdbFile, DotxdbHeader, BuildMetadata, Sample, ContigInfo,
    MetaSection, IndexOffsets, DeltaAnchor, ChainIndex, TileIndex,
    RoiResult, VerifySection, DOTXDB_MAGIC, DOTXDB_VERSION,
};
pub use serialization::{
    Serializer, SerializationConfig, SerializationFormat, DotxdbBuilder,
};