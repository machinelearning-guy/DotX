//! Masking module for DOTx
//!
//! Provides low-complexity masking and high-frequency seed suppression
//! to reduce noise in seeding and improve performance.

use crate::types::Anchor;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during masking
#[derive(Debug, Error)]
pub enum MaskError {
    #[error("Invalid sequence: {0}")]
    InvalidSequence(String),
    
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
}

pub type MaskResult<T> = Result<T, MaskError>;

/// Parameters for masking operations
#[derive(Debug, Clone)]
pub struct MaskParams {
    /// Enable low-complexity masking
    pub low_complexity_masking: bool,
    /// Window size for low-complexity detection
    pub lc_window_size: usize,
    /// Complexity threshold (entropy-based, 0.0 to 2.0)
    pub lc_complexity_threshold: f32,
    /// Enable high-frequency k-mer suppression
    pub frequency_masking: bool,
    /// Maximum frequency threshold for k-mer suppression
    pub max_kmer_frequency: u32,
    /// K-mer size for frequency analysis
    pub kmer_size: usize,
}

impl Default for MaskParams {
    fn default() -> Self {
        Self {
            low_complexity_masking: true,
            lc_window_size: 20,
            lc_complexity_threshold: 1.0,
            frequency_masking: true,
            max_kmer_frequency: 1000,
            kmer_size: 15,
        }
    }
}

/// Masking engine for sequences and anchors
pub struct Masker {
    params: MaskParams,
}

impl Masker {
    pub fn new(params: MaskParams) -> Self {
        Self { params }
    }
    
    /// Apply masking to a sequence in-place
    pub fn mask_sequence(&self, sequence: &mut [u8]) -> MaskResult<usize> {
        if sequence.is_empty() {
            return Ok(0);
        }
        
        let mut masked_positions = 0;
        
        // Apply low-complexity masking
        if self.params.low_complexity_masking {
            masked_positions += self.mask_low_complexity(sequence)?;
        }
        
        // Apply frequency masking
        if self.params.frequency_masking {
            masked_positions += self.mask_high_frequency_kmers(sequence)?;
        }
        
        Ok(masked_positions)
    }
    
    /// Mask low-complexity regions using entropy-based detection
    fn mask_low_complexity(&self, sequence: &mut [u8]) -> MaskResult<usize> {
        if sequence.len() < self.params.lc_window_size {
            return Ok(0);
        }
        
        let mut masked_positions = 0;
        
        for i in 0..=(sequence.len() - self.params.lc_window_size) {
            let window = &sequence[i..i + self.params.lc_window_size];
            let complexity = self.calculate_sequence_complexity(window)?;
            
            if complexity < self.params.lc_complexity_threshold {
                // Mark this window as low complexity
                for j in i..i + self.params.lc_window_size {
                    if sequence[j] != b'N' {
                        sequence[j] = b'N';
                        masked_positions += 1;
                    }
                }
            }
        }
        
        Ok(masked_positions)
    }
    
    /// Calculate sequence complexity using Shannon entropy
    fn calculate_sequence_complexity(&self, sequence: &[u8]) -> MaskResult<f32> {
        let mut nucleotide_counts = [0u32; 4];
        let mut total_valid = 0u32;
        
        for &nucleotide in sequence {
            match nucleotide.to_ascii_uppercase() {
                b'A' => { nucleotide_counts[0] += 1; total_valid += 1; }
                b'C' => { nucleotide_counts[1] += 1; total_valid += 1; }
                b'G' => { nucleotide_counts[2] += 1; total_valid += 1; }
                b'T' => { nucleotide_counts[3] += 1; total_valid += 1; }
                _ => {} // Skip invalid nucleotides
            }
        }
        
        if total_valid == 0 {
            return Ok(0.0);
        }
        
        let mut entropy = 0.0f32;
        for count in nucleotide_counts {
            if count > 0 {
                let probability = count as f32 / total_valid as f32;
                entropy -= probability * probability.log2();
            }
        }
        
        Ok(entropy)
    }
    
    /// Mask high-frequency k-mers that appear too frequently
    fn mask_high_frequency_kmers(&self, sequence: &mut [u8]) -> MaskResult<usize> {
        if sequence.len() < self.params.kmer_size {
            return Ok(0);
        }
        
        // First pass: count k-mer frequencies
        let kmer_frequencies = self.count_kmer_frequencies(sequence)?;
        
        // Second pass: mask high-frequency k-mers
        let mut masked_positions = 0;
        
        for i in 0..=(sequence.len() - self.params.kmer_size) {
            let kmer = &sequence[i..i + self.params.kmer_size];
            
            // Skip if already masked or contains invalid nucleotides
            if kmer.iter().any(|&b| b == b'N' || !is_valid_nucleotide(b)) {
                continue;
            }
            
            // Convert k-mer to canonical form for frequency lookup
            let canonical_kmer = self.canonicalize_kmer(kmer);
            
            if let Some(&frequency) = kmer_frequencies.get(&canonical_kmer) {
                if frequency > self.params.max_kmer_frequency {
                    // Mark this k-mer as masked
                    for j in i..i + self.params.kmer_size {
                        if sequence[j] != b'N' {
                            sequence[j] = b'N';
                            masked_positions += 1;
                        }
                    }
                }
            }
        }
        
        Ok(masked_positions)
    }
    
    /// Count k-mer frequencies in the sequence
    fn count_kmer_frequencies(&self, sequence: &[u8]) -> MaskResult<HashMap<Vec<u8>, u32>> {
        let mut frequencies = HashMap::new();
        
        for i in 0..=(sequence.len() - self.params.kmer_size) {
            let kmer = &sequence[i..i + self.params.kmer_size];
            
            // Skip k-mers with invalid nucleotides
            if kmer.iter().any(|&b| !is_valid_nucleotide(b)) {
                continue;
            }
            
            let canonical_kmer = self.canonicalize_kmer(kmer);
            *frequencies.entry(canonical_kmer).or_insert(0) += 1;
        }
        
        Ok(frequencies)
    }
    
    /// Convert k-mer to canonical form (lexicographically smaller of forward and reverse complement)
    fn canonicalize_kmer(&self, kmer: &[u8]) -> Vec<u8> {
        let reverse_complement = reverse_complement(kmer);
        if kmer <= reverse_complement.as_slice() {
            kmer.to_vec()
        } else {
            reverse_complement
        }
    }
    
    /// Filter anchors to remove those that overlap heavily with masked regions
    pub fn filter_masked_anchors(&self, anchors: Vec<Anchor>, query_seq: &[u8], target_seq: &[u8]) -> Vec<Anchor> {
        let max_masked_fraction = 0.5; // Reject anchors with >50% masked bases
        
        anchors
            .into_iter()
            .filter(|anchor| {
                let query_masked_fraction = self.calculate_masked_fraction(
                    query_seq,
                    anchor.qs as usize,
                    anchor.qe as usize,
                );
                
                let target_masked_fraction = self.calculate_masked_fraction(
                    target_seq,
                    anchor.ts as usize,
                    anchor.te as usize,
                );
                
                query_masked_fraction < max_masked_fraction &&
                target_masked_fraction < max_masked_fraction
            })
            .collect()
    }
    
    /// Calculate the fraction of masked positions in a sequence region
    fn calculate_masked_fraction(&self, sequence: &[u8], start: usize, end: usize) -> f32 {
        if start >= end || end > sequence.len() {
            return 1.0; // Invalid region, consider fully masked
        }
        
        let region = &sequence[start..end];
        let masked_count = region.iter().filter(|&&b| b == b'N').count();
        
        masked_count as f32 / region.len() as f32
    }
    
    /// Apply Dust-like low-complexity masking algorithm
    pub fn dust_mask(&self, sequence: &mut [u8], word_size: usize, threshold: f32) -> MaskResult<usize> {
        if sequence.len() < word_size {
            return Ok(0);
        }
        
        let mut masked_positions = 0;
        
        for i in 0..=(sequence.len() - word_size) {
            let window = &sequence[i..i + word_size];
            let dust_score = self.calculate_dust_score(window);
            
            if dust_score > threshold {
                // Mask this window
                for j in i..i + word_size {
                    if sequence[j] != b'N' {
                        sequence[j] = b'N';
                        masked_positions += 1;
                    }
                }
            }
        }
        
        Ok(masked_positions)
    }
    
    /// Calculate DUST score for a sequence window
    fn calculate_dust_score(&self, window: &[u8]) -> f32 {
        if window.len() < 3 {
            return 0.0;
        }
        
        let mut triplet_counts: HashMap<[u8; 3], u32> = HashMap::new();
        
        // Count all overlapping triplets
        for i in 0..=(window.len() - 3) {
            if let Ok(triplet) = window[i..i + 3].try_into() {
                // Only count valid triplets (no N's)
                if triplet.iter().all(|&b| is_valid_nucleotide(b)) {
                    *triplet_counts.entry(triplet).or_insert(0) += 1;
                }
            }
        }
        
        if triplet_counts.is_empty() {
            return 0.0;
        }
        
        // Calculate DUST score: sum of (count - 1) for each triplet
        let mut score = 0.0;
        let total_triplets = triplet_counts.values().sum::<u32>() as f32;
        
        for count in triplet_counts.values() {
            if *count > 1 {
                score += (*count - 1) as f32;
            }
        }
        
        // Normalize by window length
        score / (window.len() as f32 - 2.0).max(1.0)
    }
}

/// Check if a nucleotide is valid (A, C, G, T)
fn is_valid_nucleotide(nucleotide: u8) -> bool {
    matches!(nucleotide.to_ascii_uppercase(), b'A' | b'C' | b'G' | b'T')
}

/// Generate reverse complement of a sequence
fn reverse_complement(sequence: &[u8]) -> Vec<u8> {
    sequence
        .iter()
        .rev()
        .map(|&nucleotide| match nucleotide.to_ascii_uppercase() {
            b'A' => b'T',
            b'T' => b'A',
            b'C' => b'G',
            b'G' => b'C',
            _ => nucleotide, // Keep invalid nucleotides as-is
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_masker_creation() {
        let masker = Masker::new(MaskParams::default());
        assert!(masker.params.low_complexity_masking);
        assert!(masker.params.frequency_masking);
    }
    
    #[test]
    fn test_sequence_complexity_calculation() {
        let masker = Masker::new(MaskParams::default());
        
        // High complexity sequence
        let high_complexity = b"ATCGATCGATCG";
        let complexity = masker.calculate_sequence_complexity(high_complexity).unwrap();
        assert!(complexity > 1.5);
        
        // Low complexity sequence (all A's)
        let low_complexity = b"AAAAAAAAAAAA";
        let complexity = masker.calculate_sequence_complexity(low_complexity).unwrap();
        assert!(complexity < 0.5);
        
        // Medium complexity (two nucleotides)
        let medium_complexity = b"ATATATATATATAT";
        let complexity = masker.calculate_sequence_complexity(medium_complexity).unwrap();
        assert!(complexity > 0.5 && complexity < 1.5);
    }
    
    #[test]
    fn test_low_complexity_masking() {
        let masker = Masker::new(MaskParams {
            low_complexity_masking: true,
            lc_window_size: 10,
            lc_complexity_threshold: 1.0,
            ..Default::default()
        });
        
        let mut sequence = b"AAAAAAAAAAAATCGATCGATCGATCG".to_vec();
        let masked_count = masker.mask_low_complexity(&mut sequence).unwrap();
        
        assert!(masked_count > 0);
        
        // Check that low-complexity region was masked
        let masked_region = &sequence[0..15];
        assert!(masked_region.iter().any(|&b| b == b'N'));
    }
    
    #[test]
    fn test_kmer_frequency_counting() {
        let masker = Masker::new(MaskParams {
            kmer_size: 3,
            ..Default::default()
        });
        
        let sequence = b"ATCGATCGATCG";
        let frequencies = masker.count_kmer_frequencies(sequence).unwrap();
        
        // Should have multiple k-mers with frequencies
        assert!(!frequencies.is_empty());
        
        // Some k-mers should appear multiple times in this repetitive sequence
        assert!(frequencies.values().any(|&count| count > 1));
    }
    
    #[test]
    fn test_canonical_kmer() {
        let masker = Masker::new(MaskParams::default());
        
        let kmer = b"ATC";
        let canonical = masker.canonicalize_kmer(kmer);
        
        // ATC vs GAT (reverse complement) - ATC should be lexicographically smaller
        assert_eq!(canonical, b"ATC");
        
        let kmer2 = b"GAT";
        let canonical2 = masker.canonicalize_kmer(kmer2);
        
        // Should give the same canonical form
        assert_eq!(canonical, canonical2);
    }
    
    #[test]
    fn test_dust_score_calculation() {
        let masker = Masker::new(MaskParams::default());
        
        // High DUST score (repetitive triplets)
        let repetitive = b"ATCATCATCATC";
        let dust_score = masker.calculate_dust_score(repetitive);
        assert!(dust_score > 1.0);
        
        // Low DUST score (random sequence)
        let random = b"ATCGTACGTAGC";
        let dust_score = masker.calculate_dust_score(random);
        assert!(dust_score < 1.0);
    }
    
    #[test]
    fn test_masked_fraction_calculation() {
        let masker = Masker::new(MaskParams::default());
        
        let sequence = b"ATCNNNATCGGG";
        
        // Region with no N's
        let fraction = masker.calculate_masked_fraction(sequence, 0, 3);
        assert_eq!(fraction, 0.0);
        
        // Region with all N's
        let fraction = masker.calculate_masked_fraction(sequence, 3, 6);
        assert_eq!(fraction, 1.0);
        
        // Mixed region
        let fraction = masker.calculate_masked_fraction(sequence, 2, 8);
        assert!(fraction > 0.0 && fraction < 1.0);
    }
    
    #[test]
    fn test_full_sequence_masking() {
        let mut masker = Masker::new(MaskParams {
            low_complexity_masking: true,
            frequency_masking: false, // Disable frequency masking for this test
            lc_window_size: 8,
            lc_complexity_threshold: 1.0,
            ..Default::default()
        });
        
        let mut sequence = b"AAAAAAAAAATCGATCGATCGATCGAAAAAAAAAA".to_vec();
        let original_length = sequence.len();
        
        let masked_count = masker.mask_sequence(&mut sequence).unwrap();
        
        // Should have masked some positions
        assert!(masked_count > 0);
        
        // Sequence length should remain the same
        assert_eq!(sequence.len(), original_length);
        
        // Should contain some N's now
        assert!(sequence.iter().any(|&b| b == b'N'));
    }
    
    #[test]
    fn test_reverse_complement() {
        assert_eq!(reverse_complement(b"ATCG"), b"CGAT");
        assert_eq!(reverse_complement(b"AAAA"), b"TTTT");
        assert_eq!(reverse_complement(b"CGCG"), b"CGCG");
    }
    
    #[test]
    fn test_valid_nucleotide() {
        assert!(is_valid_nucleotide(b'A'));
        assert!(is_valid_nucleotide(b'C'));
        assert!(is_valid_nucleotide(b'G'));
        assert!(is_valid_nucleotide(b'T'));
        assert!(is_valid_nucleotide(b'a'));
        assert!(is_valid_nucleotide(b'c'));
        assert!(is_valid_nucleotide(b'g'));
        assert!(is_valid_nucleotide(b't'));
        
        assert!(!is_valid_nucleotide(b'N'));
        assert!(!is_valid_nucleotide(b'X'));
        assert!(!is_valid_nucleotide(b'-'));
    }
}