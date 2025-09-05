//! Seeding engines for DOTx
//! 
//! This module provides various algorithms for finding seed matches between sequences,
//! which form the basis for downstream chaining and alignment.

// Re-export the unified types from crate::types
pub use crate::types::{Anchor, Strand};

pub mod minimap2;
pub mod syncmer;
pub mod strobemer;
pub mod kmer;
pub mod utils;

#[cfg(test)]
pub mod tests;

/// Parameters for seeding algorithms
#[derive(Debug, Clone)]
pub struct SeedParams {
    /// K-mer size (universal parameter)
    pub k: u32,
    /// Algorithm-specific parameters
    pub algorithm_params: AlgorithmParams,
    /// Maximum frequency threshold for seed filtering
    pub max_freq: Option<u32>,
    /// Enable low-complexity masking
    pub mask_low_complexity: bool,
    /// Minimum anchor length to keep
    pub min_anchor_len: u32,
    /// Optional deterministic seed to control randomized algorithms
    pub deterministic_seed: Option<u64>,
}

impl Default for SeedParams {
    fn default() -> Self {
        Self {
            k: 15,
            algorithm_params: AlgorithmParams::Kmer,
            max_freq: Some(1000),
            mask_low_complexity: true,
            min_anchor_len: 50,
            deterministic_seed: None,
        }
    }
}

/// Algorithm-specific parameters
#[derive(Debug, Clone)]
pub enum AlgorithmParams {
    /// Direct k-mer matching
    Kmer,
    /// Minimap2 wrapper parameters
    Minimap2 {
        preset: String,
        extra_args: Vec<String>,
    },
    /// Syncmer parameters
    Syncmer {
        /// Syncmer size (s parameter)
        s: u32,
        /// Threshold (t parameter)
        t: u32,
    },
    /// Strobemer parameters
    Strobemer {
        /// Window size for linking
        window_size: u32,
        /// Maximum linking distance
        max_distance: u32,
        /// Number of strobes
        n_strobes: u32,
    },
}

/// Result type for seeding operations
pub type SeedResult<T> = Result<T, SeedError>;

/// Errors that can occur during seeding
#[derive(Debug, thiserror::Error)]
pub enum SeedError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid sequence: {0}")]
    InvalidSequence(String),
    
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
    
    #[error("External tool error: {0}")]
    ExternalTool(String),
    
    #[error("Parse error: {0}")]
    Parse(String),
}

/// Trait for seeding algorithms
pub trait Seeder {
    /// Generate seed anchors between query and target sequences
    fn seed(
        &self,
        query: &[u8],
        query_id: &str,
        target: &[u8], 
        target_id: &str,
        params: &SeedParams,
    ) -> SeedResult<Vec<Anchor>>;
    
    /// Get the name/identifier of this seeding algorithm
    fn name(&self) -> &'static str;
    
    /// Check if the seeding algorithm is available (e.g., external tools installed)
    fn is_available(&self) -> bool {
        true
    }
}

/// Factory for creating seeding engines
pub struct SeederFactory;

impl SeederFactory {
    /// Create a seeder instance based on parameters
    pub fn create(params: &SeedParams) -> Box<dyn Seeder> {
        match &params.algorithm_params {
            AlgorithmParams::Kmer => Box::new(kmer::KmerSeeder::new()),
            AlgorithmParams::Minimap2 { .. } => Box::new(minimap2::Minimap2Seeder::new()),
            AlgorithmParams::Syncmer { .. } => Box::new(syncmer::SyncmerSeeder::new()),
            AlgorithmParams::Strobemer { .. } => {
                if let Some(seed) = params.deterministic_seed {
                    Box::new(strobemer::StrobemerSeeder::with_seed(seed))
                } else {
                    Box::new(strobemer::StrobemerSeeder::new())
                }
            }
        }
    }
    
    /// List all available seeding algorithms
    pub fn available_algorithms() -> Vec<&'static str> {
        vec!["kmer", "minimap2", "syncmer", "strobemer"]
    }
}
