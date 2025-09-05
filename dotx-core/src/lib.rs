//! DOTx Core Library
//!
//! Seeding engines, IO parsers, unified types, and binary store for DOTx.

pub mod types;
pub mod io;
pub mod store;
#[cfg(feature = "seed")] pub mod seed;
pub mod chain;
#[cfg(feature = "mask")] pub mod mask;
#[cfg(feature = "dot")] pub mod dot;
#[cfg(feature = "verify")] pub mod verify;
pub mod tiles;

// Re-export commonly used types and functions
pub use types::{Anchor, Strand, Sequence};
#[cfg(feature = "seed")]
pub use seed::{SeedParams, SeedResult, SeedError, Seeder, SeederFactory, AlgorithmParams};
pub use io as formats;
pub use store::DotXStore;
pub use tiles::{DensityTile, build_density_tiles, TileBuildConfig};

/// Version information for the DOTx core library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_is_set() {
        assert!(!VERSION.is_empty());
    }
}
