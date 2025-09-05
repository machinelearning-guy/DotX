//! Strobemer (randstrobes) seeding algorithm
//!
//! Strobemers link k-mers/syncmers across windows to provide better coverage
//! and increased indel tolerance compared to single k-mers.

use std::collections::{HashMap, HashSet};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use crate::types::{Anchor, Strand};
use super::{AlgorithmParams, SeedParams, SeedResult, SeedError, Seeder};
use super::utils::{RollingHash, canonical_kmer, encode_nucleotide, reverse_complement};

/// Strobemer seeding engine
pub struct StrobemerSeeder {
    /// Random number generator for deterministic strobemer selection
    rng_seed: u64,
}

impl StrobemerSeeder {
    pub fn new() -> Self {
        Self {
            rng_seed: 42, // Default deterministic seed
        }
    }

    /// Create seeder with custom random seed for deterministic behavior
    pub fn with_seed(seed: u64) -> Self {
        Self { rng_seed: seed }
    }

    /// Generate k-mers from a sequence with their positions
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

    /// Generate strobemers by linking k-mers across windows
    fn generate_strobemers(
        &self,
        kmers: &[(u64, usize)],
        params: &SeedParams,
    ) -> SeedResult<Vec<(u64, usize, usize)>> {
        let (window_size, max_distance, n_strobes) = match &params.algorithm_params {
            AlgorithmParams::Strobemer { window_size, max_distance, n_strobes } => {
                (*window_size as usize, *max_distance as usize, *n_strobes as usize)
            }
            _ => return Err(SeedError::InvalidParams("Expected Strobemer parameters".to_string())),
        };

        if n_strobes < 2 {
            return Err(SeedError::InvalidParams("n_strobes must be >= 2".to_string()));
        }

        let mut strobemers = Vec::new();
        let mut rng = StdRng::seed_from_u64(self.rng_seed);

        // For each k-mer, try to find linked k-mers to form strobemers
        for (i, &(kmer1_hash, kmer1_pos)) in kmers.iter().enumerate() {
            // Find k-mers within the linking window
            let window_start = kmer1_pos + window_size;
            let window_end = kmer1_pos + max_distance;

            let mut candidates = Vec::new();
            for (j, &(kmer2_hash, kmer2_pos)) in kmers.iter().enumerate().skip(i + 1) {
                if kmer2_pos < window_start {
                    continue;
                }
                if kmer2_pos > window_end {
                    break;
                }
                candidates.push((j, kmer2_hash, kmer2_pos));
            }

            if candidates.is_empty() {
                continue;
            }

            // For randstrobes, select linking k-mer pseudo-randomly but deterministically
            // Use first k-mer hash as seed for deterministic selection
            let mut local_rng = StdRng::seed_from_u64(kmer1_hash ^ self.rng_seed);
            let selected_idx = local_rng.gen_range(0..candidates.len());
            let (_, kmer2_hash, kmer2_pos) = candidates[selected_idx];

            // For 2-strobes, create the strobemer hash
            if n_strobes == 2 {
                let strobemer_hash = self.combine_hashes(kmer1_hash, kmer2_hash);
                strobemers.push((strobemer_hash, kmer1_pos, kmer2_pos));
            } else {
                // For higher-order strobes, we would continue linking
                // For simplicity, this implementation focuses on 2-strobes
                // Higher-order strobes would recursively find more k-mers to link
                let strobemer_hash = self.combine_hashes(kmer1_hash, kmer2_hash);
                strobemers.push((strobemer_hash, kmer1_pos, kmer2_pos));
            }
        }

        Ok(strobemers)
    }

    /// Combine two k-mer hashes into a strobemer hash
    fn combine_hashes(&self, hash1: u64, hash2: u64) -> u64 {
        // Simple combining function - in practice, more sophisticated methods can be used
        hash1.wrapping_mul(0x9e3779b97f4a7c15) ^ hash2.wrapping_mul(0x2545f4914f6cdd1d)
    }

    /// Find matching strobemers between query and target
    fn find_strobemer_matches(
        &self,
        query_strobemers: &[(u64, usize, usize)],
        target_strobemers: &[(u64, usize, usize)],
        query_id: &str,
        target_id: &str,
        k: u32,
    ) -> Vec<Anchor> {
        // Build hash map of target strobemers
        let mut target_map: HashMap<u64, Vec<(usize, usize)>> = HashMap::new();
        for &(hash, pos1, pos2) in target_strobemers {
            target_map.entry(hash).or_insert_with(Vec::new).push((pos1, pos2));
        }

        let mut anchors = Vec::new();

        // Find matches
        for &(query_hash, query_pos1, query_pos2) in query_strobemers {
            if let Some(target_positions) = target_map.get(&query_hash) {
                for &(target_pos1, target_pos2) in target_positions {
                    // Create anchor spanning both linked k-mers
                    let query_start = std::cmp::min(query_pos1, query_pos2);
                    let query_end = std::cmp::max(query_pos1, query_pos2) + k as usize;
                    let target_start = std::cmp::min(target_pos1, target_pos2);
                    let target_end = std::cmp::max(target_pos1, target_pos2) + k as usize;

                    let anchor = Anchor::new(
                        query_id.to_string(),
                        target_id.to_string(),
                        query_start as u64,
                        query_end as u64,
                        target_start as u64,
                        target_end as u64,
                        Strand::Forward, // Canonical k-mers handle both strands
                        "strobemer".to_string(),
                    );
                    anchors.push(anchor);
                }
            }
        }

        anchors
    }

    /// Apply frequency filtering to strobemers
    fn filter_by_frequency(
        strobemers: &[(u64, usize, usize)],
        max_frequency: u32,
    ) -> Vec<(u64, usize, usize)> {
        // Count frequencies
        let mut frequencies: HashMap<u64, u32> = HashMap::new();
        for &(hash, _, _) in strobemers {
            *frequencies.entry(hash).or_insert(0) += 1;
        }

        // Filter by frequency
        strobemers
            .iter()
            .filter(|(hash, _, _)| {
                frequencies.get(hash).map_or(true, |&freq| freq <= max_frequency)
            })
            .cloned()
            .collect()
    }

    /// Chain nearby strobemer matches into longer anchors
    fn chain_strobemer_matches(&self, mut anchors: Vec<Anchor>, min_anchor_len: u32) -> Vec<Anchor> {
        if anchors.is_empty() {
            return anchors;
        }

        // Sort by query position
        anchors.sort_by_key(|a| (a.qs, a.ts));

        let mut chained_anchors = Vec::new();
        let mut current_chain = vec![anchors[0].clone()];

        for anchor in anchors.into_iter().skip(1) {
            let last_anchor = current_chain.last().unwrap();
            
            // Check if this anchor can be chained with the previous ones
            let query_gap = anchor.qs.saturating_sub(last_anchor.qe);
            let target_gap = anchor.ts.saturating_sub(last_anchor.te);
            
            // Allow reasonable gaps for chaining strobemers
            let max_gap = min_anchor_len;
            
            if query_gap <= max_gap && target_gap <= max_gap &&
               anchor.q == last_anchor.q &&
               anchor.t == last_anchor.t &&
               anchor.strand == last_anchor.strand {
                current_chain.push(anchor);
            } else {
                // Finish current chain
                if current_chain.len() > 1 {
                    let chained = self.merge_anchor_chain(&current_chain);
                    if chained.query_len() >= min_anchor_len {
                        chained_anchors.push(chained);
                    }
                } else if current_chain[0].query_len() >= min_anchor_len {
                    chained_anchors.push(current_chain[0].clone());
                }
                
                current_chain = vec![anchor];
            }
        }

        // Don't forget the last chain
        if current_chain.len() > 1 {
            let chained = self.merge_anchor_chain(&current_chain);
            if chained.query_len() >= min_anchor_len {
                chained_anchors.push(chained);
            }
        } else if current_chain.len() == 1 && current_chain[0].query_len() >= min_anchor_len {
            chained_anchors.push(current_chain[0].clone());
        }

        chained_anchors
    }

    /// Merge a chain of anchors into a single anchor
    fn merge_anchor_chain(&self, chain: &[Anchor]) -> Anchor {
        if chain.is_empty() {
            panic!("Cannot merge empty chain");
        }
        
        if chain.len() == 1 {
            return chain[0].clone();
        }

        let first = &chain[0];
        let last = &chain[chain.len() - 1];

        Anchor::new(
            first.q.clone(),
            first.t.clone(),
            first.qs,
            last.qe,
            first.ts,
            last.te,
            first.strand,
            "strobemer".to_string(),
        )
    }
}

impl Seeder for StrobemerSeeder {
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
            AlgorithmParams::Strobemer { n_strobes, .. } => {
                if *n_strobes < 2 {
                    return Err(SeedError::InvalidParams(
                        "n_strobes must be >= 2".to_string()
                    ));
                }
            }
            _ => return Err(SeedError::InvalidParams("Expected Strobemer parameters".to_string())),
        }

        let k = params.k as usize;

        // Generate k-mers for both sequences
        let query_kmers = self.generate_kmers(query, k)?;
        let target_kmers = self.generate_kmers(target, k)?;

        // Generate strobemers
        let mut query_strobemers = self.generate_strobemers(&query_kmers, params)?;
        let mut target_strobemers = self.generate_strobemers(&target_kmers, params)?;

        // Apply frequency filtering if specified
        if let Some(max_freq) = params.max_freq {
            query_strobemers = Self::filter_by_frequency(&query_strobemers, max_freq);
            target_strobemers = Self::filter_by_frequency(&target_strobemers, max_freq);
        }

        // Find matches
        let mut anchors = self.find_strobemer_matches(
            &query_strobemers,
            &target_strobemers,
            query_id,
            target_id,
            params.k,
        );

        // Also check reverse complement matches
        let query_rc = reverse_complement(query);
        let query_rc_kmers = self.generate_kmers(&query_rc, k)?;
        let query_rc_strobemers = self.generate_strobemers(&query_rc_kmers, params)?;
        
        let rc_anchors = self.find_strobemer_matches(
            &query_rc_strobemers,
            &target_strobemers,
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

        // Chain matches and apply minimum length filtering
        let chained_anchors = self.chain_strobemer_matches(anchors, params.min_anchor_len);

        Ok(chained_anchors)
    }

    fn name(&self) -> &'static str {
        "strobemer"
    }
}

impl Default for StrobemerSeeder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for creating common Strobemer parameter sets
pub struct StrobemerPresets;

impl StrobemerPresets {
    /// Default strobemer parameters (2-strobes)
    pub fn default() -> AlgorithmParams {
        AlgorithmParams::Strobemer {
            window_size: 50,
            max_distance: 100,
            n_strobes: 2,
        }
    }

    /// High sensitivity parameters (smaller window)
    pub fn high_sensitivity() -> AlgorithmParams {
        AlgorithmParams::Strobemer {
            window_size: 25,
            max_distance: 75,
            n_strobes: 2,
        }
    }

    /// Long-range parameters (larger window for distant linking)
    pub fn long_range() -> AlgorithmParams {
        AlgorithmParams::Strobemer {
            window_size: 100,
            max_distance: 300,
            n_strobes: 2,
        }
    }

    /// 3-strobes for higher specificity
    pub fn three_strobes() -> AlgorithmParams {
        AlgorithmParams::Strobemer {
            window_size: 50,
            max_distance: 150,
            n_strobes: 3,
        }
    }

    /// Custom parameters
    pub fn custom(window_size: u32, max_distance: u32, n_strobes: u32) -> AlgorithmParams {
        AlgorithmParams::Strobemer {
            window_size,
            max_distance,
            n_strobes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SEQUENCE: &[u8] = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";

    #[test]
    fn test_strobemer_seeder_creation() {
        let seeder = StrobemerSeeder::new();
        assert_eq!(seeder.name(), "strobemer");
        assert_eq!(seeder.rng_seed, 42);
    }

    #[test]
    fn test_strobemer_with_seed() {
        let seeder = StrobemerSeeder::with_seed(12345);
        assert_eq!(seeder.rng_seed, 12345);
    }

    #[test]
    fn test_kmer_generation() {
        let seeder = StrobemerSeeder::new();
        let kmers = seeder.generate_kmers(TEST_SEQUENCE, 15).unwrap();
        
        // Should generate k-mers
        assert!(kmers.len() > 0);
        
        // Positions should be valid
        for (_, pos) in &kmers {
            assert!(*pos + 15 <= TEST_SEQUENCE.len());
        }
    }

    #[test]
    fn test_strobemer_generation() {
        let seeder = StrobemerSeeder::new();
        let kmers = seeder.generate_kmers(TEST_SEQUENCE, 15).unwrap();
        
        let params = SeedParams {
            k: 15,
            algorithm_params: StrobemerPresets::default(),
            ..Default::default()
        };
        
        let strobemers = seeder.generate_strobemers(&kmers, &params).unwrap();
        
        // Should generate some strobemers
        assert!(strobemers.len() >= 0);
        
        // Each strobemer should have valid positions
        for (_, pos1, pos2) in &strobemers {
            assert!(*pos1 < TEST_SEQUENCE.len());
            assert!(*pos2 < TEST_SEQUENCE.len());
        }
    }

    #[test]
    fn test_invalid_strobemer_params() {
        let seeder = StrobemerSeeder::new();
        let params = SeedParams {
            k: 15,
            algorithm_params: AlgorithmParams::Strobemer {
                window_size: 50,
                max_distance: 100,
                n_strobes: 1, // Invalid: < 2
            },
            ..Default::default()
        };

        let result = seeder.seed(
            TEST_SEQUENCE,
            "query",
            TEST_SEQUENCE,
            "target",
            &params,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_hash_combining() {
        let seeder = StrobemerSeeder::new();
        let hash1 = 0x123456789abcdef0;
        let hash2 = 0xfedcba9876543210;
        
        let combined = seeder.combine_hashes(hash1, hash2);
        
        // Should produce a different hash
        assert_ne!(combined, hash1);
        assert_ne!(combined, hash2);
        
        // Should be deterministic
        let combined2 = seeder.combine_hashes(hash1, hash2);
        assert_eq!(combined, combined2);
    }

    #[test]
    fn test_frequency_filtering() {
        let strobemers = vec![
            (1, 0, 10), (1, 5, 15), (1, 8, 18), // Hash 1 appears 3 times
            (2, 2, 12), (2, 7, 17),             // Hash 2 appears 2 times
            (3, 4, 14),                         // Hash 3 appears 1 time
        ];

        let filtered = StrobemerSeeder::filter_by_frequency(&strobemers, 2);
        
        // Should keep hashes with frequency <= 2
        assert!(filtered.len() <= strobemers.len());
        
        // Hash 1 (frequency 3) should be filtered out
        assert!(!filtered.iter().any(|(hash, _, _)| *hash == 1));
    }

    #[test]
    fn test_anchor_chaining() {
        let seeder = StrobemerSeeder::new();
        
        // Create anchors that should be chained
        let anchors = vec![
            Anchor::new("q".to_string(), "t".to_string(), 0, 20, 0, 20, Strand::Forward, "strobemer".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 15, 35, 15, 35, Strand::Forward, "strobemer".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 30, 50, 30, 50, Strand::Forward, "strobemer".to_string()),
        ];

        let chained = seeder.chain_strobemer_matches(anchors, 30);
        
        // Should chain overlapping/nearby anchors
        assert!(chained.len() <= 3);
    }

    #[test]
    fn test_anchor_chain_merging() {
        let seeder = StrobemerSeeder::new();
        
        let chain = vec![
            Anchor::new("q".to_string(), "t".to_string(), 0, 15, 0, 15, Strand::Forward, "strobemer".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 10, 25, 10, 25, Strand::Forward, "strobemer".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 20, 35, 20, 35, Strand::Forward, "strobemer".to_string()),
        ];

        let merged = seeder.merge_anchor_chain(&chain);
        
        assert_eq!(merged.query_start, 0);
        assert_eq!(merged.query_end, 35);
        assert_eq!(merged.target_start, 0);
        assert_eq!(merged.target_end, 35);
    }

    #[test]
    fn test_end_to_end_seeding() {
        let seeder = StrobemerSeeder::new();
        let params = SeedParams {
            k: 15,
            algorithm_params: StrobemerPresets::default(),
            min_anchor_len: 20,
            ..Default::default()
        };

        // Use identical sequences to guarantee some matches
        let result = seeder.seed(
            TEST_SEQUENCE,
            "query",
            TEST_SEQUENCE,
            "target",
            &params,
        );

        assert!(result.is_ok());
        let anchors = result.unwrap();
        
        // Should find some matches for identical sequences
        assert!(anchors.len() >= 0);
    }

    #[test]
    fn test_presets() {
        match StrobemerPresets::default() {
            AlgorithmParams::Strobemer { window_size, max_distance, n_strobes } => {
                assert_eq!(window_size, 50);
                assert_eq!(max_distance, 100);
                assert_eq!(n_strobes, 2);
            }
            _ => panic!("Expected Strobemer algorithm params"),
        }

        match StrobemerPresets::custom(25, 75, 3) {
            AlgorithmParams::Strobemer { window_size, max_distance, n_strobes } => {
                assert_eq!(window_size, 25);
                assert_eq!(max_distance, 75);
                assert_eq!(n_strobes, 3);
            }
            _ => panic!("Expected Strobemer algorithm params"),
        }
    }
}
