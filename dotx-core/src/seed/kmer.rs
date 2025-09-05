//! Direct k-mer seeding mode
//!
//! This module provides simple k-mer matching without extension,
//! using fast hash-based lookup for instant structure exploration.

use std::collections::{HashMap, HashSet};
use crate::types::{Anchor, Strand};
use super::{AlgorithmParams, SeedParams, SeedResult, SeedError, Seeder};
use super::utils::{
    RollingHash, canonical_kmer, encode_nucleotide, reverse_complement, 
    filter_high_frequency_kmers, mask_low_complexity
};

/// Direct k-mer seeding engine
pub struct KmerSeeder;

impl KmerSeeder {
    pub fn new() -> Self {
        Self
    }

    /// Generate k-mers with their positions from a sequence
    fn generate_kmers(&self, sequence: &[u8], k: usize) -> SeedResult<Vec<(u64, usize)>> {
        let mut hasher = RollingHash::new(k);
        let mut kmers = Vec::new();
        let mut valid_bases = 0;

        // Process first k bases
        for i in 0..std::cmp::min(k, sequence.len()) {
            if hasher.push(sequence[i]).is_some() {
                valid_bases += 1;
            } else {
                valid_bases = 0;
                hasher.reset();
            }

            if valid_bases == k {
                let kmer_hash = canonical_kmer(hasher.hash(), k);
                kmers.push((kmer_hash, i + 1 - k));
            }
        }

        // Roll through rest of sequence
        for i in k..sequence.len() {
            if let (Some(_), Some(_)) = (
                encode_nucleotide(sequence[i - k]),
                encode_nucleotide(sequence[i])
            ) {
                if hasher.roll(sequence[i - k], sequence[i]).is_some() {
                    let kmer_hash = canonical_kmer(hasher.hash(), k);
                    kmers.push((kmer_hash, i + 1 - k));
                }
            } else {
                // Reset on invalid nucleotide
                hasher.reset();
                valid_bases = 0;
            }
        }

        Ok(kmers)
    }

    /// Find exact k-mer matches between query and target
    fn find_kmer_matches(
        &self,
        query_kmers: &[(u64, usize)],
        target_kmers: &[(u64, usize)],
        query_id: &str,
        target_id: &str,
        k: u32,
    ) -> Vec<Anchor> {
        // Build hash map of target k-mers
        let mut target_map: HashMap<u64, Vec<usize>> = HashMap::new();
        for &(hash, pos) in target_kmers {
            target_map.entry(hash).or_insert_with(Vec::new).push(pos);
        }

        let mut anchors = Vec::new();

        // Find exact matches
        for &(query_hash, query_pos) in query_kmers {
            if let Some(target_positions) = target_map.get(&query_hash) {
                for &target_pos in target_positions {
                    let anchor = Anchor::new(
                        query_id.to_string(),
                        target_id.to_string(),
                        query_pos as u64,
                        query_pos as u64 + k as u64,
                        target_pos as u64,
                        target_pos as u64 + k as u64,
                        Strand::Forward, // Canonical k-mers handle both strands
                        "kmer".to_string(),
                    );
                    anchors.push(anchor);
                }
            }
        }

        anchors
    }

    /// Filter k-mers by frequency to reduce noise
    fn filter_high_frequency_matches(
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

    /// Calculate density-based alpha values for visualization
    /// Higher density areas get lower alpha (more transparent)
    fn calculate_density_alpha(&self, anchors: &mut Vec<Anchor>, max_density: u32) {
        if anchors.is_empty() {
            return;
        }

        // Create a simple density map using binning
        let bin_size = 1000; // Base pairs per bin
        let mut density_map: HashMap<(u64, u64), u32> = HashMap::new();

        // Count anchors in each bin
        for anchor in anchors.iter() {
            let query_bin = anchor.qs / bin_size as u64;
            let target_bin = anchor.ts / bin_size as u64;
            *density_map.entry((query_bin, target_bin)).or_insert(0) += 1;
        }

        // Find maximum density for normalization
        let max_bin_density = density_map.values().max().cloned().unwrap_or(1);

        // Assign alpha values based on local density
        // This is a simple approach - more sophisticated methods could be used
        for anchor in anchors.iter_mut() {
            let query_bin = anchor.qs / bin_size as u64;
            let target_bin = anchor.ts / bin_size as u64;
            let local_density = density_map.get(&(query_bin, target_bin)).cloned().unwrap_or(1);
            
            // Higher density = lower alpha (more transparent)
            let alpha_factor = 1.0 - (local_density as f32 / max_bin_density as f32) * 0.8;
            
            // Store alpha as a custom field (would need to extend Anchor struct for this)
            // For now, we can use mapq field as a proxy for alpha information
            anchor.mapq = Some((alpha_factor * 255.0) as u8);
        }
    }

    /// Deduplicate anchors that are too close together
    fn deduplicate_nearby_anchors(&self, mut anchors: Vec<Anchor>, min_distance: u32) -> Vec<Anchor> {
        if anchors.is_empty() {
            return anchors;
        }

        // Sort by query position, then target position
        anchors.sort_by_key(|a| (a.qs, a.ts));

        let mut deduplicated = Vec::new();
        let mut last_query_pos = 0u64;
        let mut last_target_pos = 0u64;

        for anchor in anchors {
            // Check distance from last kept anchor
            let query_distance = anchor.qs.saturating_sub(last_query_pos);
            let target_distance = anchor.ts.saturating_sub(last_target_pos);

            if query_distance >= min_distance || target_distance >= min_distance || deduplicated.is_empty() {
                last_query_pos = anchor.qs;
                last_target_pos = anchor.ts;
                deduplicated.push(anchor);
            }
        }

        deduplicated
    }

    /// Process sequence with optional low-complexity masking
    fn preprocess_sequence(&self, sequence: &[u8], mask_low_complexity: bool) -> Vec<u8> {
        let mut processed = sequence.to_vec();
        
        if mask_low_complexity && sequence.len() > 50 {
            // Apply simple low-complexity masking
            super::utils::mask_low_complexity(&mut processed, 20, 1.0);
        }
        
        processed
    }
}

impl Seeder for KmerSeeder {
    fn seed(
        &self,
        query: &[u8],
        query_id: &str,
        target: &[u8],
        target_id: &str,
        params: &SeedParams,
    ) -> SeedResult<Vec<Anchor>> {
        // Validate parameters
        match &params.algorithm_params {
            AlgorithmParams::Kmer => {
                // k-mer mode doesn't have specific algorithm parameters
            }
            _ => return Err(SeedError::InvalidParams("Expected Kmer parameters".to_string())),
        }

        if params.k < 8 || params.k > 32 {
            return Err(SeedError::InvalidParams(
                "k-mer size must be between 8 and 32".to_string()
            ));
        }

        let k = params.k as usize;

        // Preprocess sequences (optional masking)
        let processed_query = self.preprocess_sequence(query, params.mask_low_complexity);
        let processed_target = self.preprocess_sequence(target, params.mask_low_complexity);

        // Generate k-mers for both sequences
        let mut query_kmers = self.generate_kmers(&processed_query, k)?;
        let mut target_kmers = self.generate_kmers(&processed_target, k)?;

        // Apply frequency filtering if specified
        if let Some(max_freq) = params.max_freq {
            let (filtered_query, filtered_target) = self.filter_high_frequency_matches(
                &query_kmers,
                &target_kmers,
                max_freq,
            );
            query_kmers = filtered_query;
            target_kmers = filtered_target;
        }

        // Find matches on forward strand
        let mut anchors = self.find_kmer_matches(
            &query_kmers,
            &target_kmers,
            query_id,
            target_id,
            params.k,
        );

        // Find matches on reverse strand
        let query_rc = reverse_complement(&processed_query);
        let query_rc_kmers = self.generate_kmers(&query_rc, k)?;
        
        let rc_anchors = self.find_kmer_matches(
            &query_rc_kmers,
            &target_kmers,
            query_id,
            target_id,
            params.k,
        );

        // Convert reverse complement matches to correct coordinates
        for mut anchor in rc_anchors {
            let query_len = query.len() as u64;
            let orig_start = anchor.qs;
            let orig_end = anchor.qe;
            
            // Reverse coordinates for reverse complement
            anchor.qs = query_len - orig_end;
            anchor.qe = query_len - orig_start;
            anchor.strand = Strand::Reverse;
            
            anchors.push(anchor);
        }

        // Deduplicate nearby matches to reduce noise
        anchors = self.deduplicate_nearby_anchors(anchors, params.k / 2);

        // Calculate density-based alpha values for visualization
        self.calculate_density_alpha(&mut anchors, 1000);

        // Filter by minimum anchor length (for k-mers, this is just the k-mer size)
        anchors.retain(|anchor| anchor.query_len() >= params.min_anchor_len);

        Ok(anchors)
    }

    fn name(&self) -> &'static str {
        "kmer"
    }
}

impl Default for KmerSeeder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for creating k-mer seeding parameter sets
pub struct KmerPresets;

impl KmerPresets {
    /// Default k-mer parameters
    pub fn default() -> AlgorithmParams {
        AlgorithmParams::Kmer
    }

    /// High sensitivity (smaller k)
    pub fn high_sensitivity() -> (AlgorithmParams, u32) {
        (AlgorithmParams::Kmer, 12)
    }

    /// Fast overview (larger k, lower density)
    pub fn fast_overview() -> (AlgorithmParams, u32) {
        (AlgorithmParams::Kmer, 21)
    }

    /// Structure exploration (medium k, moderate filtering)
    pub fn structure_exploration() -> (AlgorithmParams, u32) {
        (AlgorithmParams::Kmer, 15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_QUERY: &[u8] = b"ATCGATCGATCGATCGATCGATCGATCGATCG";
    const TEST_TARGET: &[u8] = b"GATCGATCGATCGATCGATCGATCGATCGATC";

    #[test]
    fn test_kmer_seeder_creation() {
        let seeder = KmerSeeder::new();
        assert_eq!(seeder.name(), "kmer");
    }

    #[test]
    fn test_kmer_generation() {
        let seeder = KmerSeeder::new();
        let kmers = seeder.generate_kmers(TEST_QUERY, 15).unwrap();
        
        // Should generate k-mers
        assert!(kmers.len() > 0);
        
        // Check that positions are valid
        for (_, pos) in &kmers {
            assert!(*pos + 15 <= TEST_QUERY.len());
        }
    }

    #[test]
    fn test_kmer_matching() {
        let seeder = KmerSeeder::new();
        
        let query_kmers = vec![(1, 0), (2, 5), (3, 10)];
        let target_kmers = vec![(1, 3), (2, 8), (4, 12)];
        
        let matches = seeder.find_kmer_matches(
            &query_kmers,
            &target_kmers,
            "query",
            "target",
            15,
        );

        // Should find 2 matches: hash 1 and hash 2
        assert_eq!(matches.len(), 2);
        
        for anchor in &matches {
            assert_eq!(anchor.query_id, "query");
            assert_eq!(anchor.target_id, "target");
            assert_eq!(anchor.engine_tag, "kmer");
            assert_eq!(anchor.query_len(), 15);
            assert_eq!(anchor.target_len(), 15);
        }
    }

    #[test]
    fn test_frequency_filtering() {
        let seeder = KmerSeeder::new();
        
        let query_kmers = vec![(1, 0), (1, 5), (2, 10)];
        let target_kmers = vec![(1, 0), (1, 3), (1, 7), (3, 12)];
        
        let (filtered_query, filtered_target) = seeder.filter_high_frequency_matches(
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
        let seeder = KmerSeeder::new();
        
        let anchors = vec![
            Anchor::new("q".to_string(), "t".to_string(), 0, 15, 0, 15, Strand::Forward, "kmer".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 5, 20, 5, 20, Strand::Forward, "kmer".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 50, 65, 50, 65, Strand::Forward, "kmer".to_string()),
        ];

        let deduplicated = seeder.deduplicate_nearby_anchors(anchors, 20);
        
        // Should keep first and third anchor, filter out second (too close to first)
        assert_eq!(deduplicated.len(), 2);
        assert_eq!(deduplicated[0].query_start, 0);
        assert_eq!(deduplicated[1].query_start, 50);
    }

    #[test]
    fn test_density_alpha_calculation() {
        let seeder = KmerSeeder::new();
        
        let mut anchors = vec![
            Anchor::new("q".to_string(), "t".to_string(), 0, 15, 0, 15, Strand::Forward, "kmer".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 10, 25, 10, 25, Strand::Forward, "kmer".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 5000, 5015, 5000, 5015, Strand::Forward, "kmer".to_string()),
        ];

        seeder.calculate_density_alpha(&mut anchors, 1000);
        
        // Should assign mapq values based on density
        for anchor in &anchors {
            assert!(anchor.mapq.is_some());
        }
        
        // First two anchors are in same dense region, should have lower alpha (mapq)
        // than the isolated third anchor
        assert!(anchors[0].mapq.unwrap() <= anchors[2].mapq.unwrap());
        assert!(anchors[1].mapq.unwrap() <= anchors[2].mapq.unwrap());
    }

    #[test]
    fn test_sequence_preprocessing() {
        let seeder = KmerSeeder::new();
        let sequence = b"AAAAAAAAATCGATCG".to_vec();
        
        // Test with masking enabled
        let processed = seeder.preprocess_sequence(&sequence, true);
        
        // Should be same length
        assert_eq!(processed.len(), sequence.len());
        
        // Test without masking
        let processed_no_mask = seeder.preprocess_sequence(&sequence, false);
        assert_eq!(processed_no_mask, sequence);
    }

    #[test]
    fn test_invalid_k_size() {
        let seeder = KmerSeeder::new();
        
        // Test k too small
        let params_small = SeedParams {
            k: 5, // Too small
            algorithm_params: AlgorithmParams::Kmer,
            ..Default::default()
        };
        
        let result = seeder.seed(TEST_QUERY, "query", TEST_TARGET, "target", &params_small);
        assert!(result.is_err());
        
        // Test k too large
        let params_large = SeedParams {
            k: 40, // Too large
            algorithm_params: AlgorithmParams::Kmer,
            ..Default::default()
        };
        
        let result = seeder.seed(TEST_QUERY, "query", TEST_TARGET, "target", &params_large);
        assert!(result.is_err());
    }

    #[test]
    fn test_end_to_end_seeding() {
        let seeder = KmerSeeder::new();
        let params = SeedParams {
            k: 15,
            algorithm_params: AlgorithmParams::Kmer,
            max_freq: None, // Don't filter by frequency for this test
            min_anchor_len: 15,
            ..Default::default()
        };

        let result = seeder.seed(
            TEST_QUERY,
            "query",
            TEST_QUERY, // Use identical sequences to guarantee matches
            "target",
            &params,
        );

        assert!(result.is_ok());
        let anchors = result.unwrap();
        
        // Should find matches with identical sequences
        assert!(anchors.len() > 0);
        
        // Check that all anchors have correct properties
        for anchor in &anchors {
            assert_eq!(anchor.query_id, "query");
            assert_eq!(anchor.target_id, "target");
            assert_eq!(anchor.engine_tag, "kmer");
            assert_eq!(anchor.query_len(), 15);
            assert_eq!(anchor.target_len(), 15);
            assert!(matches!(anchor.strand, Strand::Forward | Strand::Reverse));
        }
    }

    #[test]
    fn test_presets() {
        let (algo_params, _) = KmerPresets::high_sensitivity();
        assert!(matches!(algo_params, AlgorithmParams::Kmer));
        
        let (algo_params, _) = KmerPresets::fast_overview();
        assert!(matches!(algo_params, AlgorithmParams::Kmer));
        
        let default_params = KmerPresets::default();
        assert!(matches!(default_params, AlgorithmParams::Kmer));
    }
}
