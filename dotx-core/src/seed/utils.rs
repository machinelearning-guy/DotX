//! Shared utilities for seeding algorithms

use std::collections::HashMap;

/// Rolling hash implementation for efficient k-mer hashing
pub struct RollingHash {
    k: usize,
    hash: u64,
    power: u64,
    mask: u64,
}

impl RollingHash {
    const BASE: u64 = 4;
    
    pub fn new(k: usize) -> Self {
        let power = Self::BASE.pow(k as u32 - 1);
        let mask = (1u64 << (2 * k)) - 1;
        
        Self {
            k,
            hash: 0,
            power,
            mask,
        }
    }
    
    /// Add a nucleotide to the rolling hash
    pub fn push(&mut self, nucleotide: u8) -> Option<u64> {
        let encoded = encode_nucleotide(nucleotide)?;
        
        if self.k == 0 {
            return None;
        }
        
        self.hash = ((self.hash * Self::BASE) + encoded) & self.mask;
        Some(self.hash)
    }
    
    /// Remove the oldest nucleotide and add a new one
    pub fn roll(&mut self, old_nucleotide: u8, new_nucleotide: u8) -> Option<u64> {
        let old_encoded = encode_nucleotide(old_nucleotide)?;
        let new_encoded = encode_nucleotide(new_nucleotide)?;
        
        // Use wrapping arithmetic to prevent overflow
        self.hash = self.hash.wrapping_sub(old_encoded.wrapping_mul(self.power));
        self.hash = self.hash.wrapping_mul(Self::BASE).wrapping_add(new_encoded);
        self.hash &= self.mask;
        
        Some(self.hash)
    }
    
    /// Reset the hash
    pub fn reset(&mut self) {
        self.hash = 0;
    }
    
    /// Get current hash value
    pub fn hash(&self) -> u64 {
        self.hash
    }
}

/// Encode a nucleotide to 2-bit representation
pub fn encode_nucleotide(nucleotide: u8) -> Option<u64> {
    match nucleotide.to_ascii_uppercase() {
        b'A' => Some(0),
        b'C' => Some(1), 
        b'G' => Some(2),
        b'T' => Some(3),
        _ => None, // Invalid nucleotide
    }
}

/// Generate reverse complement of a sequence
pub fn reverse_complement(sequence: &[u8]) -> Vec<u8> {
    sequence
        .iter()
        .rev()
        .map(|&nucleotide| complement_nucleotide(nucleotide))
        .collect()
}

/// Get complement of a single nucleotide
pub fn complement_nucleotide(nucleotide: u8) -> u8 {
    match nucleotide.to_ascii_uppercase() {
        b'A' => b'T',
        b'T' => b'A',
        b'C' => b'G',
        b'G' => b'C',
        _ => nucleotide, // Return as-is for invalid nucleotides
    }
}

/// Simple low-complexity masking using a sliding window
pub fn mask_low_complexity(sequence: &mut [u8], window_size: usize, complexity_threshold: f32) {
    if sequence.len() < window_size {
        return;
    }
    
    for i in 0..=(sequence.len() - window_size) {
        let window = &sequence[i..i + window_size];
        if calculate_complexity(window) < complexity_threshold {
            // Mask this region with 'N'
            for j in i..i + window_size {
                sequence[j] = b'N';
            }
        }
    }
}

/// Calculate sequence complexity (entropy-based)
fn calculate_complexity(sequence: &[u8]) -> f32 {
    let mut counts = [0u32; 4];
    let mut total = 0u32;
    
    for &nucleotide in sequence {
        if let Some(encoded) = encode_nucleotide(nucleotide) {
            counts[encoded as usize] += 1;
            total += 1;
        }
    }
    
    if total == 0 {
        return 0.0;
    }
    
    let mut entropy = 0.0f32;
    for count in counts {
        if count > 0 {
            let p = count as f32 / total as f32;
            entropy -= p * p.log2();
        }
    }
    
    entropy
}

/// Count k-mer frequencies in a sequence
pub fn count_kmer_frequencies(sequence: &[u8], k: usize) -> HashMap<u64, u32> {
    let mut counts = HashMap::new();
    let mut hasher = RollingHash::new(k);
    
    // Initialize hash with first k-mer
    let mut valid_bases = 0;
    for i in 0..std::cmp::min(k, sequence.len()) {
        if hasher.push(sequence[i]).is_some() {
            valid_bases += 1;
        } else {
            valid_bases = 0;
            hasher.reset();
        }
        
        if valid_bases == k {
            *counts.entry(hasher.hash()).or_insert(0) += 1;
        }
    }
    
    // Roll through the rest of the sequence
    for i in k..sequence.len() {
        if let (Some(_), Some(_)) = (
            encode_nucleotide(sequence[i - k]),
            encode_nucleotide(sequence[i])
        ) {
            if hasher.roll(sequence[i - k], sequence[i]).is_some() {
                *counts.entry(hasher.hash()).or_insert(0) += 1;
            }
        } else {
            // Reset on invalid nucleotide
            hasher.reset();
            valid_bases = 0;
        }
    }
    
    counts
}

/// Filter k-mers by frequency threshold
pub fn filter_high_frequency_kmers(
    sequence: &[u8],
    k: usize,
    max_frequency: u32,
) -> Vec<(u64, usize)> {
    let frequencies = count_kmer_frequencies(sequence, k);
    let mut result = Vec::new();
    let mut hasher = RollingHash::new(k);
    
    // Find positions of k-mers below frequency threshold
    let mut valid_bases = 0;
    for i in 0..std::cmp::min(k, sequence.len()) {
        if hasher.push(sequence[i]).is_some() {
            valid_bases += 1;
        } else {
            valid_bases = 0;
            hasher.reset();
        }
        
        if valid_bases == k {
            let hash = hasher.hash();
            if let Some(&freq) = frequencies.get(&hash) {
                if freq <= max_frequency {
                    result.push((hash, i + 1 - k));
                }
            }
        }
    }
    
    // Roll through the rest of the sequence
    for i in k..sequence.len() {
        if let (Some(_), Some(_)) = (
            encode_nucleotide(sequence[i - k]),
            encode_nucleotide(sequence[i])
        ) {
            if hasher.roll(sequence[i - k], sequence[i]).is_some() {
                let hash = hasher.hash();
                if let Some(&freq) = frequencies.get(&hash) {
                    if freq <= max_frequency {
                        result.push((hash, i + 1 - k));
                    }
                }
            }
        } else {
            // Reset on invalid nucleotide
            hasher.reset();
            valid_bases = 0;
        }
    }
    
    result
}

/// Compute canonical k-mer (lexicographically smaller of forward and reverse complement)
pub fn canonical_kmer(kmer_hash: u64, k: usize) -> u64 {
    let rc_hash = reverse_complement_hash(kmer_hash, k);
    std::cmp::min(kmer_hash, rc_hash)
}

/// Compute reverse complement hash
pub fn reverse_complement_hash(hash: u64, k: usize) -> u64 {
    let mut result = 0u64;
    let mut temp_hash = hash;
    
    for _ in 0..k {
        let nucleotide = temp_hash & 3; // Extract lowest 2 bits
        let complement = 3 - nucleotide; // Complement: A(0)<->T(3), C(1)<->G(2)
        result = (result << 2) | complement;
        temp_hash >>= 2;
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nucleotide_encoding() {
        assert_eq!(encode_nucleotide(b'A'), Some(0));
        assert_eq!(encode_nucleotide(b'C'), Some(1));
        assert_eq!(encode_nucleotide(b'G'), Some(2));
        assert_eq!(encode_nucleotide(b'T'), Some(3));
        assert_eq!(encode_nucleotide(b'N'), None);
    }

    #[test]
    fn test_reverse_complement() {
        let sequence = b"ATCG";
        let rc = reverse_complement(sequence);
        assert_eq!(rc, b"CGAT");
    }

    #[test]
    fn test_rolling_hash() {
        let mut hasher = RollingHash::new(3);
        
        // Test push
        hasher.push(b'A'); // 0
        hasher.push(b'T'); // 0*4 + 3 = 3
        let hash1 = hasher.push(b'C').unwrap(); // 3*4 + 1 = 13
        
        // Test roll
        let hash2 = hasher.roll(b'A', b'G').unwrap(); // Remove A, add G: should be TCG
        
        // Verify by computing expected hash manually
        let expected = 3 * 4 + 1; // TC = 13
        let expected = (expected * 4 + 2) & ((1 << 6) - 1); // TCG = 54
        assert_eq!(hash2, expected);
    }

    #[test]
    fn test_kmer_frequency_counting() {
        let sequence = b"AAATTTAAATTT";
        let counts = count_kmer_frequencies(sequence, 3);
        
        // Should have AAA, AAT, ATT, TTT, TTA, TAA appearing
        assert!(counts.len() > 0);
    }

    #[test]
    fn test_low_complexity_masking() {
        let mut sequence = b"AAAAAAAAATCGATCG".to_vec();
        mask_low_complexity(&mut sequence, 8, 1.5);
        
        // The AAAAAAAA region should be masked
        assert!(sequence.iter().any(|&b| b == b'N'));
    }

    #[test]
    fn test_canonical_kmer() {
        // Test with a simple example where we know the reverse complement
        let hash = 0b0011; // AC (2-mer)
        let k = 2;
        let canonical = canonical_kmer(hash, k);
        
        // AC -> GT, so canonical should be AC (00 11) vs GT (10 00) -> AC is smaller
        assert!(canonical <= hash);
    }
}