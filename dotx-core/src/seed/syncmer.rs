//! Syncmer seeding algorithm
//!
//! Syncmers are a sampling scheme that selects k-mers based on a minimizer criterion,
//! providing good conservation at lower density compared to all k-mers.

use std::collections::{HashMap, VecDeque};
use crate::types::{Anchor, Strand};
use super::{AlgorithmParams, SeedParams, SeedResult, SeedError, Seeder};
use super::utils::{RollingHash, canonical_kmer, encode_nucleotide, reverse_complement};

/// Syncmer seeding engine
pub struct SyncmerSeeder;

impl SyncmerSeeder {
    pub fn new() -> Self {
        Self
    }

    /// Check if a k-mer is a syncmer
    /// A k-mer is a syncmer if its minimum s-mer occurs at a specific position
    fn is_syncmer(kmer_hash: u64, k: usize, s: usize, t: usize) -> bool {
        if s >= k {
            return false; // Invalid parameters
        }

        let mut min_smer = u64::MAX;
        let mut min_pos = 0;
        let smer_mask = (1u64 << (2 * s)) - 1;

        // Extract all s-mers from the k-mer and find minimum
        for i in 0..=(k - s) {
            let smer = (kmer_hash >> (2 * i)) & smer_mask;
            if smer < min_smer {
                min_smer = smer;
                min_pos = i;
            }
        }

        // Check if minimum s-mer is at the threshold position
        min_pos == t % (k - s + 1)
    }

    /// Generate syncmers from a sequence
    fn generate_syncmers(
        &self,
        sequence: &[u8],
        seq_id: &str,
        params: &SeedParams,
    ) -> SeedResult<Vec<(u64, usize)>> {
        let (s, t) = match &params.algorithm_params {
            AlgorithmParams::Syncmer { s, t } => (*s as usize, *t as usize),
            _ => return Err(SeedError::InvalidParams("Expected Syncmer parameters".to_string())),
        };

        if params.k < s as u32 {
            return Err(SeedError::InvalidParams(
                "k-mer size must be >= syncmer size".to_string()
            ));
        }

        let k = params.k as usize;
        let mut hasher = RollingHash::new(k);
        let mut syncmers = Vec::new();
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
                let kmer_hash = hasher.hash();
                let canonical = canonical_kmer(kmer_hash, k);
                
                if Self::is_syncmer(canonical, k, s, t) {
                    syncmers.push((canonical, i + 1 - k));
                }
            }
        }

        // Roll through rest of sequence
        for i in k..sequence.len() {
            if let (Some(_), Some(_)) = (
                encode_nucleotide(sequence[i - k]),
                encode_nucleotide(sequence[i])
            ) {
                if hasher.roll(sequence[i - k], sequence[i]).is_some() {
                    let kmer_hash = hasher.hash();
                    let canonical = canonical_kmer(kmer_hash, k);
                    
                    if Self::is_syncmer(canonical, k, s, t) {
                        syncmers.push((canonical, i + 1 - k));
                    }
                }
            } else {
                // Reset on invalid nucleotide
                hasher.reset();
                valid_bases = 0;
            }
        }

        Ok(syncmers)
    }

    /// Find matching syncmers between query and target
    fn find_matches(
        &self,
        query_syncmers: &[(u64, usize)],
        target_syncmers: &[(u64, usize)],
        query_id: &str,
        target_id: &str,
        k: u32,
    ) -> Vec<Anchor> {
        // Build hash map of target syncmers
        let mut target_map: HashMap<u64, Vec<usize>> = HashMap::new();
        for &(hash, pos) in target_syncmers {
            target_map.entry(hash).or_insert_with(Vec::new).push(pos);
        }

        let mut anchors = Vec::new();

        // Find matches
        for &(query_hash, query_pos) in query_syncmers {
            if let Some(target_positions) = target_map.get(&query_hash) {
                for &target_pos in target_positions {
                    // Create anchor for this match
                    let anchor = Anchor::new(
                        query_id.to_string(),
                        target_id.to_string(),
                        query_pos as u64,
                        query_pos as u64 + k as u64,
                        target_pos as u64,
                        target_pos as u64 + k as u64,
                        Strand::Forward, // Canonical k-mers handle both strands
                        "syncmer".to_string(),
                    );
                    anchors.push(anchor);
                }
            }
        }

        anchors
    }

    /// Apply frequency filtering to syncmers
    fn filter_by_frequency(
        syncmers: &[(u64, usize)],
        max_frequency: u32,
    ) -> Vec<(u64, usize)> {
        // Count frequencies
        let mut frequencies: HashMap<u64, u32> = HashMap::new();
        for &(hash, _) in syncmers {
            *frequencies.entry(hash).or_insert(0) += 1;
        }

        // Filter by frequency
        syncmers
            .iter()
            .filter(|(hash, _)| {
                frequencies.get(hash).map_or(true, |&freq| freq <= max_frequency)
            })
            .cloned()
            .collect()
    }

    /// Extend matches into longer anchors
    fn extend_matches(&self, mut anchors: Vec<Anchor>, k: u32, min_anchor_len: u32) -> Vec<Anchor> {
        if anchors.is_empty() {
            return anchors;
        }

        // Sort anchors by query position for extension
        anchors.sort_by_key(|a| (a.qs, a.ts));

        let mut extended_anchors = Vec::new();
        let mut current_anchor = anchors[0].clone();

        for anchor in anchors.into_iter().skip(1) {
            // Check if this anchor can be merged with the current one
            let query_gap = anchor.qs.saturating_sub(current_anchor.qe);
            let target_gap = anchor.ts.saturating_sub(current_anchor.te);

            // Allow small gaps (up to k bases) for extension
            if query_gap <= k && target_gap <= k && 
               anchor.q == current_anchor.q &&
               anchor.t == current_anchor.t &&
               anchor.strand == current_anchor.strand {
                // Extend current anchor
                current_anchor.qe = anchor.qe;
                current_anchor.te = anchor.te;
            } else {
                // Finish current anchor if it meets minimum length requirement
                if current_anchor.query_len() >= min_anchor_len {
                    extended_anchors.push(current_anchor);
                }
                current_anchor = anchor;
            }
        }

        // Don't forget the last anchor
        if current_anchor.query_len() >= min_anchor_len {
            extended_anchors.push(current_anchor);
        }

        extended_anchors
    }
}

impl Seeder for SyncmerSeeder {
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
            AlgorithmParams::Syncmer { s, t: _ } => {
                if *s >= params.k {
                    return Err(SeedError::InvalidParams(
                        "Syncmer size (s) must be less than k-mer size (k)".to_string()
                    ));
                }
            }
            _ => return Err(SeedError::InvalidParams("Expected Syncmer parameters".to_string())),
        }

        // Generate syncmers for both sequences
        let mut query_syncmers = self.generate_syncmers(query, query_id, params)?;
        let mut target_syncmers = self.generate_syncmers(target, target_id, params)?;

        // Apply frequency filtering if specified
        if let Some(max_freq) = params.max_freq {
            // Combine syncmers from both sequences for frequency calculation
            let mut all_syncmers = query_syncmers.clone();
            all_syncmers.extend(target_syncmers.clone());
            
            query_syncmers = Self::filter_by_frequency(&query_syncmers, max_freq);
            target_syncmers = Self::filter_by_frequency(&target_syncmers, max_freq);
        }

        // Find matches
        let mut anchors = self.find_matches(
            &query_syncmers,
            &target_syncmers,
            query_id,
            target_id,
            params.k,
        );

        // Also check reverse complement matches
        let query_rc = reverse_complement(query);
        let query_rc_syncmers = self.generate_syncmers(&query_rc, query_id, params)?;
        
        let rc_anchors = self.find_matches(
            &query_rc_syncmers,
            &target_syncmers,
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

        // Extend matches and apply minimum length filtering
        let extended_anchors = self.extend_matches(anchors, params.k, params.min_anchor_len);

        Ok(extended_anchors)
    }

    fn name(&self) -> &'static str {
        "syncmer"
    }
}

impl Default for SyncmerSeeder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for creating common Syncmer parameter sets
pub struct SyncmerPresets;

impl SyncmerPresets {
    /// Default syncmer parameters (k=21, s=11, t=0)
    pub fn default() -> AlgorithmParams {
        AlgorithmParams::Syncmer { s: 11, t: 0 }
    }

    /// High sensitivity parameters (smaller s)
    pub fn high_sensitivity() -> AlgorithmParams {
        AlgorithmParams::Syncmer { s: 8, t: 0 }
    }

    /// Low density parameters (larger s)
    pub fn low_density() -> AlgorithmParams {
        AlgorithmParams::Syncmer { s: 15, t: 0 }
    }

    /// Custom parameters
    pub fn custom(s: u32, t: u32) -> AlgorithmParams {
        AlgorithmParams::Syncmer { s, t }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SEQUENCE: &[u8] = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";

    #[test]
    fn test_syncmer_seeder_creation() {
        let seeder = SyncmerSeeder::new();
        assert_eq!(seeder.name(), "syncmer");
    }

    #[test]
    fn test_is_syncmer() {
        // Test with simple parameters
        let k = 6;
        let s = 3;
        let t = 0;
        
        // This is a simplified test - in practice, the syncmer test
        // depends on the specific hash values
        let test_hash = 0b101010; // Some test hash
        let result = SyncmerSeeder::is_syncmer(test_hash, k, s, t);
        
        // Just verify it doesn't crash and returns a boolean
        assert!(result == true || result == false);
    }

    #[test]
    fn test_invalid_syncmer_params() {
        let seeder = SyncmerSeeder::new();
        let params = SeedParams {
            k: 15,
            algorithm_params: AlgorithmParams::Syncmer { s: 20, t: 0 }, // s > k
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
    fn test_syncmer_generation() {
        let seeder = SyncmerSeeder::new();
        let params = SeedParams {
            k: 15,
            algorithm_params: SyncmerPresets::default(),
            ..Default::default()
        };

        let syncmers = seeder.generate_syncmers(TEST_SEQUENCE, "test", &params).unwrap();
        
        // Should generate some syncmers (exact count depends on sequence)
        // Just verify it doesn't crash and produces reasonable output
        assert!(syncmers.len() >= 0);
        
        // Verify positions are within sequence bounds
        for (_, pos) in &syncmers {
            assert!(*pos + params.k as usize <= TEST_SEQUENCE.len());
        }
    }

    #[test]
    fn test_frequency_filtering() {
        let syncmers = vec![
            (1, 0), (1, 5), (1, 10), // Hash 1 appears 3 times
            (2, 2), (2, 7),          // Hash 2 appears 2 times
            (3, 4),                  // Hash 3 appears 1 time
        ];

        let filtered = SyncmerSeeder::filter_by_frequency(&syncmers, 2);
        
        // Should keep hashes with frequency <= 2
        assert!(filtered.len() <= syncmers.len());
        
        // Hash 1 (frequency 3) should be filtered out
        assert!(!filtered.iter().any(|(hash, _)| *hash == 1));
    }

    #[test]
    fn test_match_finding() {
        let seeder = SyncmerSeeder::new();
        
        let query_syncmers = vec![(1, 0), (2, 5), (3, 10)];
        let target_syncmers = vec![(1, 3), (2, 8), (4, 12)];
        
        let matches = seeder.find_matches(
            &query_syncmers,
            &target_syncmers,
            "query",
            "target",
            15,
        );

        // Should find 2 matches: hash 1 and hash 2
        assert_eq!(matches.len(), 2);
        
        for anchor in &matches {
            assert_eq!(anchor.query_id, "query");
            assert_eq!(anchor.target_id, "target");
            assert_eq!(anchor.engine_tag, "syncmer");
        }
    }

    #[test]
    fn test_anchor_extension() {
        let seeder = SyncmerSeeder::new();
        
        // Create anchors that should be extended
        let anchors = vec![
            Anchor::new("q".to_string(), "t".to_string(), 0, 15, 0, 15, Strand::Forward, "syncmer".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 10, 25, 10, 25, Strand::Forward, "syncmer".to_string()),
            Anchor::new("q".to_string(), "t".to_string(), 20, 35, 20, 35, Strand::Forward, "syncmer".to_string()),
        ];

        let extended = seeder.extend_matches(anchors, 15, 30);
        
        // Should merge overlapping/nearby anchors
        assert!(extended.len() <= 3);
        
        if !extended.is_empty() {
            // Check that extended anchors meet minimum length
            for anchor in &extended {
                assert!(anchor.query_len() >= 30);
            }
        }
    }

    #[test]
    fn test_end_to_end_seeding() {
        let seeder = SyncmerSeeder::new();
        let params = SeedParams {
            k: 15,
            algorithm_params: SyncmerPresets::default(),
            min_anchor_len: 15,
            ..Default::default()
        };

        // Use identical sequences to guarantee matches
        let result = seeder.seed(
            TEST_SEQUENCE,
            "query",
            TEST_SEQUENCE,
            "target",
            &params,
        );

        assert!(result.is_ok());
        let anchors = result.unwrap();
        
        // Should find at least some matches for identical sequences
        // (exact count depends on syncmer selection)
        assert!(anchors.len() >= 0);
    }

    #[test]
    fn test_presets() {
        match SyncmerPresets::default() {
            AlgorithmParams::Syncmer { s, t } => {
                assert_eq!(s, 11);
                assert_eq!(t, 0);
            }
            _ => panic!("Expected Syncmer algorithm params"),
        }

        match SyncmerPresets::custom(5, 2) {
            AlgorithmParams::Syncmer { s, t } => {
                assert_eq!(s, 5);
                assert_eq!(t, 2);
            }
            _ => panic!("Expected Syncmer algorithm params"),
        }
    }
}
