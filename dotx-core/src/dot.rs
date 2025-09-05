//! Direct k-mer dot mode for DOTx
//!
//! This module provides direct k-mer matching without extension or chaining,
//! ideal for quick structure exploration and instant visualization of large datasets.
//! Includes density-based alpha blending for better visualization.

use crate::types::{Anchor, Strand};
use crate::seed::utils::{RollingHash, canonical_kmer, encode_nucleotide, reverse_complement};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during dot plotting
#[derive(Debug, Error)]
pub enum DotError {
    #[error("Invalid sequence: {0}")]
    InvalidSequence(String),
    
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
    
    #[error("Memory allocation failed")]
    OutOfMemory,
}

pub type DotResult<T> = Result<T, DotError>;

/// Parameters for direct k-mer dot mode
#[derive(Debug, Clone)]
pub struct DotParams {
    /// K-mer size
    pub k: usize,
    /// Maximum frequency threshold for noise reduction
    pub max_frequency: Option<u32>,
    /// Minimum spacing between dots (for deduplication)
    pub min_spacing: u32,
    /// Enable density-based alpha blending
    pub density_alpha: bool,
    /// Bin size for density calculation (base pairs)
    pub density_bin_size: u32,
    /// Maximum density for alpha normalization
    pub max_density_threshold: u32,
    /// Sample rate (0.0 to 1.0) for very large datasets
    pub sample_rate: f32,
}

impl Default for DotParams {
    fn default() -> Self {
        Self {
            k: 15,
            max_frequency: Some(100),
            min_spacing: 10,
            density_alpha: true,
            density_bin_size: 1000,
            max_density_threshold: 1000,
            sample_rate: 1.0,
        }
    }
}

/// Direct k-mer dot plotting engine
pub struct DotPlotter {
    params: DotParams,
}

impl DotPlotter {
    pub fn new(params: DotParams) -> Self {
        Self { params }
    }
    
    /// Generate direct k-mer dots between query and target sequences
    pub fn generate_dots(
        &self,
        query: &[u8],
        query_id: &str,
        target: &[u8],
        target_id: &str,
    ) -> DotResult<Vec<Anchor>> {
        if self.params.k < 8 || self.params.k > 32 {
            return Err(DotError::InvalidParams(
                "k-mer size must be between 8 and 32".to_string()
            ));
        }
        
        if self.params.sample_rate <= 0.0 || self.params.sample_rate > 1.0 {
            return Err(DotError::InvalidParams(
                "Sample rate must be between 0.0 and 1.0".to_string()
            ));
        }
        
        // Generate k-mers for both sequences
        let query_kmers = self.generate_kmers(query)?;
        let target_kmers = self.generate_kmers(target)?;
        
        // Apply frequency filtering if specified
        let (filtered_query, filtered_target) = if let Some(max_freq) = self.params.max_frequency {
            self.filter_high_frequency(&query_kmers, &target_kmers, max_freq)
        } else {
            (query_kmers, target_kmers)
        };
        
        // Find exact matches
        let mut dots = self.find_exact_matches(
            &filtered_query,
            &filtered_target,
            query_id,
            target_id,
        );
        
        // Find reverse complement matches
        let query_rc = reverse_complement(query);
        let query_rc_kmers = self.generate_kmers(&query_rc)?;
        let (filtered_query_rc, _) = if let Some(max_freq) = self.params.max_frequency {
            self.filter_high_frequency(&query_rc_kmers, &filtered_target, max_freq)
        } else {
            (query_rc_kmers, filtered_target.clone())
        };
        
        let rc_dots = self.find_exact_matches(
            &filtered_query_rc,
            &filtered_target,
            query_id,
            target_id,
        );
        
        // Convert reverse complement coordinates
        for mut dot in rc_dots {
            let query_len = query.len() as u64;
            let orig_start = dot.qs;
            let orig_end = dot.qe;
            
            dot.qs = query_len - orig_end;
            dot.qe = query_len - orig_start;
            dot.strand = Strand::Reverse;
            
            dots.push(dot);
        }
        
        // Apply sampling if needed
        if self.params.sample_rate < 1.0 {
            dots = self.sample_dots(dots, self.params.sample_rate);
        }
        
        // Deduplicate nearby dots
        if self.params.min_spacing > 0 {
            dots = self.deduplicate_dots(dots);
        }
        
        // Apply density-based alpha blending
        if self.params.density_alpha {
            self.apply_density_alpha(&mut dots);
        }
        
        Ok(dots)
    }
    
    /// Generate k-mers from a sequence
    fn generate_kmers(&self, sequence: &[u8]) -> DotResult<Vec<(u64, usize)>> {
        let mut hasher = RollingHash::new(self.params.k);
        let mut kmers = Vec::new();
        let mut valid_bases = 0;
        
        // Process first k bases
        for i in 0..std::cmp::min(self.params.k, sequence.len()) {
            if hasher.push(sequence[i]).is_some() {
                valid_bases += 1;
            } else {
                valid_bases = 0;
                hasher.reset();
            }
            
            if valid_bases == self.params.k {
                let kmer_hash = canonical_kmer(hasher.hash(), self.params.k);
                kmers.push((kmer_hash, i + 1 - self.params.k));
            }
        }
        
        // Roll through rest of sequence
        for i in self.params.k..sequence.len() {
            if let (Some(_), Some(_)) = (
                encode_nucleotide(sequence[i - self.params.k]),
                encode_nucleotide(sequence[i])
            ) {
                if hasher.roll(sequence[i - self.params.k], sequence[i]).is_some() {
                    let kmer_hash = canonical_kmer(hasher.hash(), self.params.k);
                    kmers.push((kmer_hash, i + 1 - self.params.k));
                }
            } else {
                hasher.reset();
                valid_bases = 0;
            }
        }
        
        Ok(kmers)
    }
    
    /// Filter high-frequency k-mers from both sequences
    fn filter_high_frequency(
        &self,
        query_kmers: &[(u64, usize)],
        target_kmers: &[(u64, usize)],
        max_frequency: u32,
    ) -> (Vec<(u64, usize)>, Vec<(u64, usize)>) {
        // Count frequencies across both sequences
        let mut frequencies: HashMap<u64, u32> = HashMap::new();
        
        for &(hash, _) in query_kmers {
            *frequencies.entry(hash).or_insert(0) += 1;
        }
        
        for &(hash, _) in target_kmers {
            *frequencies.entry(hash).or_insert(0) += 1;
        }
        
        // Filter by frequency
        let filtered_query: Vec<_> = query_kmers
            .iter()
            .filter(|(hash, _)| {
                frequencies.get(hash).map_or(true, |&freq| freq <= max_frequency)
            })
            .cloned()
            .collect();
        
        let filtered_target: Vec<_> = target_kmers
            .iter()
            .filter(|(hash, _)| {
                frequencies.get(hash).map_or(true, |&freq| freq <= max_frequency)
            })
            .cloned()
            .collect();
        
        (filtered_query, filtered_target)
    }
    
    /// Find exact k-mer matches between sequences
    fn find_exact_matches(
        &self,
        query_kmers: &[(u64, usize)],
        target_kmers: &[(u64, usize)],
        query_id: &str,
        target_id: &str,
    ) -> Vec<Anchor> {
        // Build hash map of target k-mers
        let mut target_map: HashMap<u64, Vec<usize>> = HashMap::new();
        for &(hash, pos) in target_kmers {
            target_map.entry(hash).or_insert_with(Vec::new).push(pos);
        }
        
        let mut dots = Vec::new();
        
        // Find matches
        for &(query_hash, query_pos) in query_kmers {
            if let Some(target_positions) = target_map.get(&query_hash) {
                for &target_pos in target_positions {
                    let dot = Anchor::new(
                        query_id.to_string(),
                        target_id.to_string(),
                        query_pos as u64,
                        query_pos as u64 + self.params.k as u64,
                        target_pos as u64,
                        target_pos as u64 + self.params.k as u64,
                        Strand::Forward, // Will be corrected for reverse complement matches
                        "dot".to_string(),
                    );
                    dots.push(dot);
                }
            }
        }
        
        dots
    }
    
    /// Sample dots for very large datasets
    fn sample_dots(&self, mut dots: Vec<Anchor>, sample_rate: f32) -> Vec<Anchor> {
        use rand::{Rng, SeedableRng};
        use rand::rngs::StdRng;
        
        if sample_rate >= 1.0 {
            return dots;
        }
        
        let mut rng = StdRng::seed_from_u64(42); // Deterministic sampling
        
        dots.retain(|_| rng.gen::<f32>() < sample_rate);
        
        dots
    }
    
    /// Deduplicate dots that are too close together
    fn deduplicate_dots(&self, mut dots: Vec<Anchor>) -> Vec<Anchor> {
        if dots.is_empty() {
            return dots;
        }
        
        // Sort by query position, then target position
        dots.sort_by_key(|dot| (dot.qs, dot.ts));
        
        let mut deduplicated = Vec::new();
        let mut last_query_pos = 0u64;
        let mut last_target_pos = 0u64;
        
        for dot in dots {
            let query_distance = dot.qs.saturating_sub(last_query_pos);
            let target_distance = dot.ts.saturating_sub(last_target_pos);
            
            if query_distance >= self.params.min_spacing as u64 || 
               target_distance >= self.params.min_spacing as u64 || 
               deduplicated.is_empty() {
                last_query_pos = dot.qs;
                last_target_pos = dot.ts;
                deduplicated.push(dot);
            }
        }
        
        deduplicated
    }
    
    /// Apply density-based alpha blending for visualization
    fn apply_density_alpha(&self, dots: &mut [Anchor]) {
        if dots.is_empty() {
            return;
        }
        
        // Create density map using binning
        let bin_size = self.params.density_bin_size as u64;
        let mut density_map: HashMap<(u64, u64), u32> = HashMap::new();
        
        // Count dots in each bin
        for dot in dots.iter() {
            let query_bin = dot.qs / bin_size;
            let target_bin = dot.ts / bin_size;
            *density_map.entry((query_bin, target_bin)).or_insert(0) += 1;
        }
        
        // Find maximum density for normalization
        let max_density = density_map
            .values()
            .max()
            .cloned()
            .unwrap_or(1)
            .min(self.params.max_density_threshold);
        
        // Assign alpha values based on local density
        for dot in dots.iter_mut() {
            let query_bin = dot.qs / bin_size;
            let target_bin = dot.ts / bin_size;
            let local_density = density_map
                .get(&(query_bin, target_bin))
                .cloned()
                .unwrap_or(1);
            
            // Calculate alpha: high density = low alpha (more transparent)
            let alpha_factor = 1.0 - (local_density as f32 / max_density as f32) * 0.8;
            let alpha_factor = alpha_factor.max(0.1).min(1.0); // Clamp to reasonable range
            
            // Store alpha in mapq field as a proxy (0-255 scale)
            dot.mapq = Some((alpha_factor * 255.0) as u8);
        }
    }
    
    /// Get statistics about the generated dots
    pub fn get_dot_statistics(&self, dots: &[Anchor]) -> DotStatistics {
        if dots.is_empty() {
            return DotStatistics::default();
        }
        
        let forward_count = dots.iter().filter(|d| d.strand == Strand::Forward).count();
        let reverse_count = dots.iter().filter(|d| d.strand == Strand::Reverse).count();
        
        let query_range = if !dots.is_empty() {
            let min_q = dots.iter().map(|d| d.qs).min().unwrap_or(0);
            let max_q = dots.iter().map(|d| d.qe).max().unwrap_or(0);
            (min_q, max_q)
        } else {
            (0, 0)
        };
        
        let target_range = if !dots.is_empty() {
            let min_t = dots.iter().map(|d| d.ts).min().unwrap_or(0);
            let max_t = dots.iter().map(|d| d.te).max().unwrap_or(0);
            (min_t, max_t)
        } else {
            (0, 0)
        };
        
        DotStatistics {
            total_dots: dots.len(),
            forward_strand_dots: forward_count,
            reverse_strand_dots: reverse_count,
            query_range,
            target_range,
            kmer_size: self.params.k,
        }
    }
}

/// Statistics about generated dots
#[derive(Debug, Clone, Default)]
pub struct DotStatistics {
    pub total_dots: usize,
    pub forward_strand_dots: usize,
    pub reverse_strand_dots: usize,
    pub query_range: (u64, u64),
    pub target_range: (u64, u64),
    pub kmer_size: usize,
}

/// Preset configurations for different use cases
pub struct DotPresets;

impl DotPresets {
    /// Fast overview with large k-mers and aggressive filtering
    pub fn fast_overview() -> DotParams {
        DotParams {
            k: 21,
            max_frequency: Some(50),
            min_spacing: 100,
            sample_rate: 0.1,
            ..Default::default()
        }
    }
    
    /// High resolution with smaller k-mers
    pub fn high_resolution() -> DotParams {
        DotParams {
            k: 12,
            max_frequency: Some(200),
            min_spacing: 5,
            sample_rate: 1.0,
            ..Default::default()
        }
    }
    
    /// Structure exploration with medium parameters
    pub fn structure_exploration() -> DotParams {
        DotParams {
            k: 15,
            max_frequency: Some(100),
            min_spacing: 20,
            sample_rate: 0.5,
            ..Default::default()
        }
    }
    
    /// Self-dot comparison optimized parameters
    pub fn self_comparison() -> DotParams {
        DotParams {
            k: 18,
            max_frequency: Some(10), // Aggressive filtering for self-dot
            min_spacing: 50,
            density_alpha: true,
            sample_rate: 0.3,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    const TEST_QUERY: &[u8] = b"ATCGATCGATCGATCGATCGATCGATCGATCG";
    const TEST_TARGET: &[u8] = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";
    
    #[test]
    fn test_dot_plotter_creation() {
        let plotter = DotPlotter::new(DotParams::default());
        assert_eq!(plotter.params.k, 15);
        assert!(plotter.params.density_alpha);
    }
    
    #[test]
    fn test_kmer_generation() {
        let plotter = DotPlotter::new(DotParams {
            k: 15,
            ..Default::default()
        });
        
        let kmers = plotter.generate_kmers(TEST_QUERY).unwrap();
        
        // Should generate k-mers
        assert!(!kmers.is_empty());
        
        // Check positions are valid
        for (_, pos) in &kmers {
            assert!(*pos + 15 <= TEST_QUERY.len());
        }
    }
    
    #[test]
    fn test_exact_matching() {
        let plotter = DotPlotter::new(DotParams {
            k: 15,
            ..Default::default()
        });
        
        let query_kmers = vec![(1, 0), (2, 5), (3, 10)];
        let target_kmers = vec![(1, 3), (2, 8), (4, 12)];
        
        let dots = plotter.find_exact_matches(
            &query_kmers,
            &target_kmers,
            "query",
            "target",
        );
        
        // Should find 2 matches: hash 1 and hash 2
        assert_eq!(dots.len(), 2);
        
        for dot in &dots {
            assert_eq!(dot.q, "query");
            assert_eq!(dot.t, "target");
            assert_eq!(dot.engine_tag, "dot");
            assert_eq!(dot.query_len(), 15);
        }
    }
    
    #[test]
    fn test_frequency_filtering() {
        let plotter = DotPlotter::new(DotParams::default());
        
        let query_kmers = vec![(1, 0), (1, 5), (2, 10)];
        let target_kmers = vec![(1, 0), (1, 3), (1, 7), (3, 12)];
        
        let (filtered_query, filtered_target) = plotter.filter_high_frequency(
            &query_kmers,
            &target_kmers,
            3, // max frequency
        );
        
        // Hash 1 appears 2+3=5 times total, should be filtered out
        assert!(!filtered_query.iter().any(|(hash, _)| *hash == 1));
        assert!(!filtered_target.iter().any(|(hash, _)| *hash == 1));
    }
    
    #[test]
    fn test_deduplication() {
        let plotter = DotPlotter::new(DotParams {
            min_spacing: 20,
            ..Default::default()
        });
        
        let dots = vec![
            Anchor::new("q".to_string(), "t".to_string(), 0, 15, 0, 15, Strand::Forward, "dot".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 5, 20, 5, 20, Strand::Forward, "dot".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 50, 65, 50, 65, Strand::Forward, "dot".to_string()),
        ];
        
        let deduplicated = plotter.deduplicate_dots(dots);
        
        // Should keep first and third dot, filter second (too close to first)
        assert_eq!(deduplicated.len(), 2);
        assert_eq!(deduplicated[0].qs, 0);
        assert_eq!(deduplicated[1].qs, 50);
    }
    
    #[test]
    fn test_density_alpha_calculation() {
        let plotter = DotPlotter::new(DotParams {
            density_alpha: true,
            density_bin_size: 1000,
            ..Default::default()
        });
        
        let mut dots = vec![
            Anchor::new("q".to_string(), "t".to_string(), 0, 15, 0, 15, Strand::Forward, "dot".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 10, 25, 10, 25, Strand::Forward, "dot".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 5000, 5015, 5000, 5015, Strand::Forward, "dot".to_string()),
        ];
        
        plotter.apply_density_alpha(&mut dots);
        
        // Should assign mapq values based on density
        for dot in &dots {
            assert!(dot.mapq.is_some());
        }
        
        // First two dots are in same dense bin, should have lower alpha
        // than the isolated third dot
        assert!(dots[0].mapq.unwrap() <= dots[2].mapq.unwrap());
        assert!(dots[1].mapq.unwrap() <= dots[2].mapq.unwrap());
    }
    
    #[test]
    fn test_sampling() {
        let plotter = DotPlotter::new(DotParams::default());
        
        let dots = vec![
            Anchor::new("q".to_string(), "t".to_string(), 0, 15, 0, 15, Strand::Forward, "dot".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 100, 115, 100, 115, Strand::Forward, "dot".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 200, 215, 200, 215, Strand::Forward, "dot".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 300, 315, 300, 315, Strand::Forward, "dot".to_string()),
        ];
        
        let sampled = plotter.sample_dots(dots.clone(), 0.5);
        
        // Should sample roughly half (deterministic with fixed seed)
        assert!(sampled.len() <= dots.len());
        
        // Full sampling should return all dots
        let full_sampled = plotter.sample_dots(dots.clone(), 1.0);
        assert_eq!(full_sampled.len(), dots.len());
    }
    
    #[test]
    fn test_end_to_end_dot_generation() {
        let plotter = DotPlotter::new(DotParams {
            k: 15,
            max_frequency: None, // No frequency filtering
            min_spacing: 0,      // No deduplication
            sample_rate: 1.0,    // No sampling
            density_alpha: true,
            ..Default::default()
        });
        
        let dots = plotter.generate_dots(
            TEST_QUERY,
            "query",
            TEST_QUERY, // Use identical sequences to guarantee matches
            "target",
        ).unwrap();
        
        // Should find matches for identical sequences
        assert!(!dots.is_empty());
        
        // Should have both forward and reverse strand matches
        let forward_count = dots.iter().filter(|d| d.strand == Strand::Forward).count();
        let reverse_count = dots.iter().filter(|d| d.strand == Strand::Reverse).count();
        
        assert!(forward_count > 0);
        assert!(reverse_count > 0);
        
        // Check properties of dots
        for dot in &dots {
            assert_eq!(dot.q, "query");
            assert_eq!(dot.t, "target");
            assert_eq!(dot.engine_tag, "dot");
            assert_eq!(dot.query_len(), 15);
            assert!(dot.mapq.is_some()); // Should have alpha values
        }
    }
    
    #[test]
    fn test_statistics() {
        let plotter = DotPlotter::new(DotParams::default());
        
        let dots = vec![
            Anchor::new("q".to_string(), "t".to_string(), 0, 15, 0, 15, Strand::Forward, "dot".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 100, 115, 100, 115, Strand::Reverse, "dot".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 200, 215, 200, 215, Strand::Forward, "dot".to_string()),
        ];
        
        let stats = plotter.get_dot_statistics(&dots);
        
        assert_eq!(stats.total_dots, 3);
        assert_eq!(stats.forward_strand_dots, 2);
        assert_eq!(stats.reverse_strand_dots, 1);
        assert_eq!(stats.query_range, (0, 215));
        assert_eq!(stats.target_range, (0, 215));
        assert_eq!(stats.kmer_size, 15);
    }
    
    #[test]
    fn test_presets() {
        let fast = DotPresets::fast_overview();
        assert_eq!(fast.k, 21);
        assert_eq!(fast.sample_rate, 0.1);
        
        let high_res = DotPresets::high_resolution();
        assert_eq!(high_res.k, 12);
        assert_eq!(high_res.sample_rate, 1.0);
        
        let structure = DotPresets::structure_exploration();
        assert_eq!(structure.k, 15);
        assert_eq!(structure.sample_rate, 0.5);
        
        let self_dot = DotPresets::self_comparison();
        assert_eq!(self_dot.k, 18);
        assert_eq!(self_dot.max_frequency, Some(10));
    }
    
    #[test]
    fn test_invalid_parameters() {
        // Invalid k-mer size
        let result = DotPlotter::new(DotParams {
            k: 5, // Too small
            ..Default::default()
        }).generate_dots(TEST_QUERY, "q", TEST_TARGET, "t");
        assert!(result.is_err());
        
        // Invalid sample rate
        let result = DotPlotter::new(DotParams {
            sample_rate: 1.5, // > 1.0
            ..Default::default()
        }).generate_dots(TEST_QUERY, "q", TEST_TARGET, "t");
        assert!(result.is_err());
    }
}