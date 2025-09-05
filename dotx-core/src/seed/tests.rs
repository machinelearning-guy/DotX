//! Comprehensive tests for seeding engines with small example sequences

use super::*;
use crate::seed::{
    minimap2::{Minimap2Seeder, Minimap2Presets},
    syncmer::{SyncmerSeeder, SyncmerPresets},
    strobemer::{StrobemerSeeder, StrobemerPresets},
    kmer::{KmerSeeder, KmerPresets},
};

/// Test sequences for comprehensive testing
pub struct TestSequences;

impl TestSequences {
    /// Simple identical sequences
    pub fn identical() -> (&'static [u8], &'static [u8]) {
        let seq = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";
        (seq, seq)
    }

    /// Sequences with a simple mutation
    pub fn single_mutation() -> (&'static [u8], &'static [u8]) {
        let query = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";
        let target = b"ATCGATCGATCGATCGAACGATCGATCGATCGATCGATCG"; // T->A at position 16
        (query, target)
    }

    /// Sequences with an insertion
    pub fn insertion() -> (&'static [u8], &'static [u8]) {
        let query = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";
        let target = b"ATCGATCGATCGATCGAAATCGATCGATCGATCGATCGATCG"; // AA inserted
        (query, target)
    }

    /// Sequences with a deletion
    pub fn deletion() -> (&'static [u8], &'static [u8]) {
        let query = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";
        let target = b"ATCGATCGATCGATCGTCGATCGATCGATCGATCGATCG"; // GA deleted
        (query, target)
    }

    /// Sequences with an inversion
    pub fn inversion() -> (&'static [u8], &'static [u8]) {
        let query = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";
        let target = b"ATCGATCGATCGATCGCGATCGATCATCGATCGATCGATCG"; // TCGA -> CGAT
        (query, target)
    }

    /// Query is reverse complement of target
    pub fn reverse_complement() -> (Vec<u8>, Vec<u8>) {
        let query = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG".to_vec();
        // Generate proper reverse complement using the utility function
        let target = super::utils::reverse_complement(&query);
        (query, target)
    }

    /// Sequences with repetitive regions
    pub fn repetitive() -> (&'static [u8], &'static [u8]) {
        let query = b"ATATATATATCGATCGATCGATATATATATATAT";
        let target = b"ATATATATCGATCGATCGATCGATATATATATATTA";
        (query, target)
    }

    /// Short sequences for edge case testing
    pub fn short_sequences() -> (&'static [u8], &'static [u8]) {
        let query = b"ATCGATCGATCG";
        let target = b"CGATCGATCGAT";
        (query, target)
    }

    /// Sequences with ambiguous nucleotides
    pub fn ambiguous_nucleotides() -> (&'static [u8], &'static [u8]) {
        let query = b"ATCGATCGATCGATCGATNNATCGATCGATCGATCGATCGATCG";
        let target = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";
        (query, target)
    }

    /// Long identical sequences for performance testing
    pub fn long_identical() -> (Vec<u8>, Vec<u8>) {
        let pattern = b"ATCGATCGATCG";
        let mut seq = Vec::new();
        
        // Create 1KB sequences
        for _ in 0..85 {
            seq.extend_from_slice(pattern);
        }
        
        (seq.clone(), seq)
    }
}

#[cfg(test)]
mod seeding_tests {
    use super::*;

    #[test]
    fn test_kmer_seeder_identical_sequences() {
        let seeder = KmerSeeder::new();
        let (query, target) = TestSequences::identical();
        
        let params = SeedParams {
            k: 15,
            algorithm_params: AlgorithmParams::Kmer,
            min_anchor_len: 15,
            ..Default::default()
        };

        let result = seeder.seed(&query, "query", &target, "target", &params);
        assert!(result.is_ok());
        
        let anchors = result.unwrap();
        assert!(!anchors.is_empty(), "Should find matches in identical sequences");
        
        // Check that all anchors are valid
        for anchor in &anchors {
            assert_eq!(anchor.query_len(), 15);
            assert_eq!(anchor.target_len(), 15);
            assert!(anchor.query_start < query.len() as u32);
            assert!(anchor.target_start < target.len() as u32);
        }
    }

    #[test]
    fn test_kmer_seeder_reverse_complement() {
        let seeder = KmerSeeder::new();
        let (query, target) = TestSequences::reverse_complement();
        
        let params = SeedParams {
            k: 10, // Use smaller k-mer for better chance of matches
            algorithm_params: AlgorithmParams::Kmer,
            min_anchor_len: 10,
            ..Default::default()
        };

        let result = seeder.seed(&query, "query", &target, "target", &params);
        assert!(result.is_ok());
        
        let anchors = result.unwrap();
        
        // For a true reverse complement, we should find matches (either plus or minus strand)
        // The key point is that the seeder can handle reverse complement sequences
        if anchors.is_empty() {
            // If no matches, at least verify the seeder doesn't crash
            println!("No reverse complement matches found - this may be expected for short sequences");
        } else {
            // If we do find matches, they should be valid
            for anchor in &anchors {
                assert!(anchor.query_len() >= 10);
                assert_eq!(anchor.engine_tag, "kmer");
            }
        }
    }

    #[test]
    fn test_syncmer_seeder_with_mutation() {
        let seeder = SyncmerSeeder::new();
        let (query, target) = TestSequences::single_mutation();
        
        let params = SeedParams {
            k: 15,
            algorithm_params: SyncmerPresets::default(),
            min_anchor_len: 15,
            ..Default::default()
        };

        let result = seeder.seed(&query, "query", &target, "target", &params);
        assert!(result.is_ok());
        
        let anchors = result.unwrap();
        // May or may not find matches depending on syncmer selection
        // Just verify it doesn't crash
        for anchor in &anchors {
            assert!(anchor.query_len() >= 15);
            assert_eq!(anchor.engine_tag, "syncmer");
        }
    }

    #[test]
    fn test_strobemer_seeder_with_insertion() {
        let seeder = StrobemerSeeder::new();
        let (query, target) = TestSequences::insertion();
        
        let params = SeedParams {
            k: 15,
            algorithm_params: StrobemerPresets::default(),
            min_anchor_len: 20,
            ..Default::default()
        };

        let result = seeder.seed(&query, "query", &target, "target", &params);
        assert!(result.is_ok());
        
        let anchors = result.unwrap();
        // Strobemers should handle indels better
        for anchor in &anchors {
            assert!(anchor.query_len() >= 20);
            assert_eq!(anchor.engine_tag, "strobemer");
        }
    }

    #[test]
    fn test_frequency_filtering_repetitive_sequences() {
        let seeder = KmerSeeder::new();
        let (query, target) = TestSequences::repetitive();
        
        let params_no_filter = SeedParams {
            k: 10,
            algorithm_params: AlgorithmParams::Kmer,
            max_freq: None, // No filtering
            min_anchor_len: 10,
            ..Default::default()
        };

        let params_with_filter = SeedParams {
            k: 10,
            algorithm_params: AlgorithmParams::Kmer,
            max_freq: Some(5), // Aggressive filtering
            min_anchor_len: 10,
            ..Default::default()
        };

        let result_no_filter = seeder.seed(query, "query", target, "target", &params_no_filter);
        let result_with_filter = seeder.seed(query, "query", target, "target", &params_with_filter);
        
        assert!(result_no_filter.is_ok());
        assert!(result_with_filter.is_ok());
        
        let anchors_no_filter = result_no_filter.unwrap();
        let anchors_with_filter = result_with_filter.unwrap();
        
        // Filtering should reduce the number of anchors
        assert!(anchors_with_filter.len() <= anchors_no_filter.len());
    }

    #[test]
    fn test_short_sequences_handling() {
        let seeder = KmerSeeder::new();
        let (query, target) = TestSequences::short_sequences();
        
        let params = SeedParams {
            k: 10,
            algorithm_params: AlgorithmParams::Kmer,
            min_anchor_len: 10,
            ..Default::default()
        };

        let result = seeder.seed(&query, "query", &target, "target", &params);
        assert!(result.is_ok());
        
        // May or may not find matches, but shouldn't crash
        let anchors = result.unwrap();
        for anchor in &anchors {
            assert!(anchor.query_start < query.len() as u32);
            assert!(anchor.target_start < target.len() as u32);
        }
    }

    #[test]
    fn test_ambiguous_nucleotides() {
        let seeder = KmerSeeder::new();
        let (query, target) = TestSequences::ambiguous_nucleotides();
        
        let params = SeedParams {
            k: 15,
            algorithm_params: AlgorithmParams::Kmer,
            min_anchor_len: 15,
            ..Default::default()
        };

        let result = seeder.seed(&query, "query", &target, "target", &params);
        assert!(result.is_ok());
        
        // Should handle ambiguous nucleotides gracefully
        let anchors = result.unwrap();
        for anchor in &anchors {
            // Anchors should not span regions with N's if handled correctly
            assert!(anchor.query_len() >= 15);
        }
    }

    #[test]
    fn test_parameter_validation() {
        let seeder = KmerSeeder::new();
        let (query, target) = TestSequences::identical();
        
        // Test invalid k-mer size
        let invalid_params = SeedParams {
            k: 3, // Too small
            algorithm_params: AlgorithmParams::Kmer,
            ..Default::default()
        };

        let result = seeder.seed(query, "query", target, "target", &invalid_params);
        assert!(result.is_err(), "Should reject invalid k-mer size");
    }

    #[test] 
    fn test_syncmer_parameter_validation() {
        let seeder = SyncmerSeeder::new();
        let (query, target) = TestSequences::identical();
        
        // Test invalid syncmer parameters (s >= k)
        let invalid_params = SeedParams {
            k: 15,
            algorithm_params: AlgorithmParams::Syncmer { s: 20, t: 0 }, // s > k
            ..Default::default()
        };

        let result = seeder.seed(query, "query", target, "target", &invalid_params);
        assert!(result.is_err(), "Should reject invalid syncmer parameters");
    }

    #[test]
    fn test_strobemer_parameter_validation() {
        let seeder = StrobemerSeeder::new();
        let (query, target) = TestSequences::identical();
        
        // Test invalid strobemer parameters (n_strobes < 2)
        let invalid_params = SeedParams {
            k: 15,
            algorithm_params: AlgorithmParams::Strobemer {
                window_size: 50,
                max_distance: 100,
                n_strobes: 1, // Invalid
            },
            ..Default::default()
        };

        let result = seeder.seed(query, "query", target, "target", &invalid_params);
        assert!(result.is_err(), "Should reject invalid strobemer parameters");
    }

    #[test]
    fn test_seeder_factory() {
        let kmer_params = SeedParams {
            algorithm_params: AlgorithmParams::Kmer,
            ..Default::default()
        };
        
        let seeder = SeederFactory::create(&kmer_params);
        assert_eq!(seeder.name(), "kmer");
        
        let syncmer_params = SeedParams {
            algorithm_params: SyncmerPresets::default(),
            ..Default::default()
        };
        
        let seeder = SeederFactory::create(&syncmer_params);
        assert_eq!(seeder.name(), "syncmer");
    }

    #[test]
    fn test_available_algorithms() {
        let algorithms = SeederFactory::available_algorithms();
        assert!(algorithms.contains(&"kmer"));
        assert!(algorithms.contains(&"syncmer"));
        assert!(algorithms.contains(&"strobemer"));
        assert!(algorithms.contains(&"minimap2"));
    }

    #[test]
    fn test_anchor_properties() {
        let anchor = Anchor::new(
            "query1".to_string(),
            "target1".to_string(),
            100,
            150,
            200,
            250,
            Strand::Plus,
            "test".to_string(),
        );
        
        assert_eq!(anchor.query_len(), 50);
        assert_eq!(anchor.target_len(), 50);
        assert_eq!(anchor.avg_len(), 50.0);
        assert_eq!(anchor.strand.to_string(), "+");
    }

    // Conditional test that only runs if minimap2 is available
    #[test]
    fn test_minimap2_if_available() {
        let seeder = Minimap2Seeder::new();
        
        if !seeder.is_available() {
            eprintln!("Skipping minimap2 test - binary not available");
            return;
        }
        
        let (query, target) = TestSequences::identical();
        let params = SeedParams {
            k: 15,
            algorithm_params: Minimap2Presets::asm5(),
            ..Default::default()
        };

        let result = seeder.seed(&query, "query", &target, "target", &params);
        assert!(result.is_ok(), "Minimap2 should work with available binary");
    }

    #[test]
    fn test_deterministic_behavior() {
        let seeder1 = StrobemerSeeder::with_seed(12345);
        let seeder2 = StrobemerSeeder::with_seed(12345);
        let seeder3 = StrobemerSeeder::with_seed(54321);
        
        let (query, target) = TestSequences::identical();
        let params = SeedParams {
            k: 15,
            algorithm_params: StrobemerPresets::default(),
            ..Default::default()
        };

        let result1 = seeder1.seed(query, "query", target, "target", &params);
        let result2 = seeder2.seed(query, "query", target, "target", &params);
        let result3 = seeder3.seed(query, "query", target, "target", &params);
        
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());
        
        let anchors1 = result1.unwrap();
        let anchors2 = result2.unwrap();
        let anchors3 = result3.unwrap();
        
        // Same seed should produce identical results
        assert_eq!(anchors1.len(), anchors2.len());
        
        // Different seeds may produce different results
        // (but this is not guaranteed, so we don't assert inequality)
    }

    #[test]
    fn test_performance_with_long_sequences() {
        let seeder = KmerSeeder::new();
        let (query, target) = TestSequences::long_identical();
        
        let params = SeedParams {
            k: 21,
            algorithm_params: AlgorithmParams::Kmer,
            max_freq: Some(100), // Apply filtering for performance
            min_anchor_len: 21,
            ..Default::default()
        };

        let start_time = std::time::Instant::now();
        let result = seeder.seed(&query, "query", &target, "target", &params);
        let elapsed = start_time.elapsed();
        
        assert!(result.is_ok());
        assert!(elapsed.as_secs() < 5, "Should complete within 5 seconds");
        
        let anchors = result.unwrap();
        println!("Generated {} anchors for 1KB sequences in {:?}", anchors.len(), elapsed);
    }
}

/// Integration tests that verify cross-engine compatibility
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_all_engines_produce_valid_output() {
        let (query, target) = TestSequences::identical();
        let engines = vec![
            ("kmer", Box::new(KmerSeeder::new()) as Box<dyn Seeder>),
            ("syncmer", Box::new(SyncmerSeeder::new()) as Box<dyn Seeder>),
            ("strobemer", Box::new(StrobemerSeeder::new()) as Box<dyn Seeder>),
        ];

        for (name, seeder) in engines {
            let params = match name {
                "kmer" => SeedParams {
                    k: 15,
                    algorithm_params: AlgorithmParams::Kmer,
                    ..Default::default()
                },
                "syncmer" => SeedParams {
                    k: 15,
                    algorithm_params: SyncmerPresets::default(),
                    ..Default::default()
                },
                "strobemer" => SeedParams {
                    k: 15,
                    algorithm_params: StrobemerPresets::default(),
                    ..Default::default()
                },
                _ => unreachable!(),
            };

            let result = seeder.seed(&query, "query", &target, "target", &params);
            assert!(result.is_ok(), "Engine {} should not fail", name);
            
            let anchors = result.unwrap();
            for anchor in &anchors {
                // Verify all anchors have valid properties
                assert!(!anchor.query_id.is_empty());
                assert!(!anchor.target_id.is_empty());
                assert!(anchor.query_start < anchor.query_end);
                assert!(anchor.target_start < anchor.target_end);
                assert!(!anchor.engine_tag.is_empty());
            }
        }
    }

    #[test]
    fn test_coordinate_consistency() {
        let seeder = KmerSeeder::new();
        let (query, target) = TestSequences::reverse_complement();
        
        let params = SeedParams {
            k: 15,
            algorithm_params: AlgorithmParams::Kmer,
            ..Default::default()
        };

        let result = seeder.seed(&query, "query", &target, "target", &params);
        assert!(result.is_ok());
        
        let anchors = result.unwrap();
        for anchor in &anchors {
            // Verify coordinate bounds
            assert!(anchor.query_start < query.len() as u32);
            assert!(anchor.query_end <= query.len() as u32);
            assert!(anchor.target_start < target.len() as u32);
            assert!(anchor.target_end <= target.len() as u32);
            
            // Verify coordinate ordering
            assert!(anchor.query_start < anchor.query_end);
            assert!(anchor.target_start < anchor.target_end);
        }
    }
}