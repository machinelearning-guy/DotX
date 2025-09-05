//! Verification module for DOTx
//!
//! Provides local exact alignment for refining seed anchors using
//! a WFA2-inspired approach for high accuracy identity calculation.

use crate::types::{Anchor, Strand};
use std::cmp::{max, min};
use thiserror::Error;

/// Errors that can occur during verification
#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Invalid sequence: {0}")]
    InvalidSequence(String),
    
    #[error("Alignment failed: {0}")]
    AlignmentFailed(String),
    
    #[error("Out of memory during alignment")]
    OutOfMemory,
    
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
}

pub type VerifyResult<T> = Result<T, VerifyError>;

/// Parameters for the verification algorithm
#[derive(Debug, Clone)]
pub struct VerifyParams {
    /// Match score
    pub match_score: i32,
    /// Mismatch penalty
    pub mismatch_penalty: i32,
    /// Gap open penalty
    pub gap_open: i32,
    /// Gap extend penalty
    pub gap_extend: i32,
    /// Maximum allowed edit distance as fraction of length
    pub max_edit_distance: f64,
    /// Bandwidth for banded alignment
    pub bandwidth: usize,
}

impl Default for VerifyParams {
    fn default() -> Self {
        Self {
            match_score: 2,
            mismatch_penalty: -1,
            gap_open: -2,
            gap_extend: -1,
            max_edit_distance: 0.3,
            bandwidth: 100,
        }
    }
}

/// Result of verification alignment
#[derive(Debug, Clone)]
pub struct AlignmentResult {
    /// Identity percentage (0.0 to 100.0)
    pub identity: f32,
    /// Number of matches
    pub matches: u32,
    /// Number of mismatches  
    pub mismatches: u32,
    /// Number of insertions
    pub insertions: u32,
    /// Number of deletions
    pub deletions: u32,
    /// Alignment score
    pub score: i32,
    /// Query alignment start position
    pub query_start: u64,
    /// Query alignment end position  
    pub query_end: u64,
    /// Target alignment start position
    pub target_start: u64,
    /// Target alignment end position
    pub target_end: u64,
}

impl AlignmentResult {
    /// Calculate identity as a percentage
    pub fn calculate_identity(&self) -> f32 {
        let total_ops = self.matches + self.mismatches + self.insertions + self.deletions;
        if total_ops == 0 {
            return 0.0;
        }
        (self.matches as f32 / total_ops as f32) * 100.0
    }
    
    /// Get total edit distance
    pub fn edit_distance(&self) -> u32 {
        self.mismatches + self.insertions + self.deletions
    }
    
    /// Get alignment length
    pub fn alignment_length(&self) -> u64 {
        max(
            self.query_end - self.query_start,
            self.target_end - self.target_start,
        )
    }
}

/// Verification engine for local exact alignment
pub struct Verifier {
    params: VerifyParams,
}

impl Verifier {
    pub fn new(params: VerifyParams) -> Self {
        Self { params }
    }
    
    /// Verify an anchor by performing local alignment
    pub fn verify_anchor(
        &self,
        anchor: &Anchor,
        query_seq: &[u8],
        target_seq: &[u8],
    ) -> VerifyResult<AlignmentResult> {
        // Extract sequences for the anchor region with some padding
        let padding = 50; // base pairs of padding on each side
        
        let query_start = anchor.qs.saturating_sub(padding);
        let query_end = min(anchor.qe + padding, query_seq.len() as u64);
        let target_start = anchor.ts.saturating_sub(padding);
        let target_end = min(anchor.te + padding, target_seq.len() as u64);
        
        if query_start >= query_end || target_start >= target_end {
            return Err(VerifyError::InvalidSequence(
                "Invalid coordinate ranges".to_string(),
            ));
        }
        
        let query_subseq = &query_seq[query_start as usize..query_end as usize];
        let target_subseq = &target_seq[target_start as usize..target_end as usize];
        
        // Handle reverse complement for reverse strand
        let (aligned_query, aligned_target) = match anchor.strand {
            Strand::Forward => (query_subseq.to_vec(), target_subseq.to_vec()),
            Strand::Reverse => {
                (query_subseq.to_vec(), reverse_complement(target_subseq))
            }
        };
        
        // Perform banded alignment
        self.banded_align(&aligned_query, &aligned_target, query_start, target_start)
    }
    
    /// Perform banded local alignment using a simplified WFA approach
    fn banded_align(
        &self,
        query: &[u8],
        target: &[u8],
        query_offset: u64,
        target_offset: u64,
    ) -> VerifyResult<AlignmentResult> {
        let m = query.len();
        let n = target.len();
        
        if m == 0 || n == 0 {
            return Err(VerifyError::InvalidSequence("Empty sequences".to_string()));
        }
        
        // Use banded DP for efficiency
        let bandwidth = min(self.params.bandwidth, max(m, n));
        
        // DP table: dp[i][j] represents best score ending at query[i], target[j]
        let mut dp = vec![vec![i32::MIN / 2; n + 1]; m + 1];
        let mut traceback = vec![vec![TracebackOp::None; n + 1]; m + 1];
        
        // Initialize
        dp[0][0] = 0;
        
        // Fill first row (deletions in query)
        for j in 1..=min(bandwidth, n) {
            dp[0][j] = self.params.gap_open + (j as i32 - 1) * self.params.gap_extend;
            traceback[0][j] = TracebackOp::Delete;
        }
        
        // Fill first column (insertions in query)
        for i in 1..=min(bandwidth, m) {
            dp[i][0] = self.params.gap_open + (i as i32 - 1) * self.params.gap_extend;
            traceback[i][0] = TracebackOp::Insert;
        }
        
        // Fill DP table with banding
        for i in 1..=m {
            let j_start = max(1, i.saturating_sub(bandwidth));
            let j_end = min(n, i + bandwidth);
            
            for j in j_start..=j_end {
                let mut best_score = i32::MIN / 2;
                let mut best_op = TracebackOp::None;
                
                // Match/mismatch
                if i > 0 && j > 0 {
                    let score = if query[i - 1] == target[j - 1] {
                        self.params.match_score
                    } else {
                        self.params.mismatch_penalty
                    };
                    
                    let candidate = dp[i - 1][j - 1] + score;
                    if candidate > best_score {
                        best_score = candidate;
                        best_op = TracebackOp::Match;
                    }
                }
                
                // Insertion (gap in target)
                if i > 0 && dp[i - 1][j] != i32::MIN / 2 {
                    let gap_penalty = if traceback[i - 1][j] == TracebackOp::Insert {
                        self.params.gap_extend
                    } else {
                        self.params.gap_open
                    };
                    
                    let candidate = dp[i - 1][j] + gap_penalty;
                    if candidate > best_score {
                        best_score = candidate;
                        best_op = TracebackOp::Insert;
                    }
                }
                
                // Deletion (gap in query)
                if j > 0 && dp[i][j - 1] != i32::MIN / 2 {
                    let gap_penalty = if traceback[i][j - 1] == TracebackOp::Delete {
                        self.params.gap_extend
                    } else {
                        self.params.gap_open
                    };
                    
                    let candidate = dp[i][j - 1] + gap_penalty;
                    if candidate > best_score {
                        best_score = candidate;
                        best_op = TracebackOp::Delete;
                    }
                }
                
                dp[i][j] = best_score;
                traceback[i][j] = best_op;
            }
        }
        
        // Find best alignment end position
        let mut best_score = i32::MIN;
        let mut best_i = m;
        let mut best_j = n;
        
        for i in 0..=m {
            for j in 0..=n {
                if dp[i][j] > best_score {
                    best_score = dp[i][j];
                    best_i = i;
                    best_j = j;
                }
            }
        }
        
        // Traceback to count operations
        let mut matches = 0u32;
        let mut mismatches = 0u32;
        let mut insertions = 0u32;
        let mut deletions = 0u32;
        
        let mut i = best_i;
        let mut j = best_j;
        let end_i = i;
        let end_j = j;
        
        while i > 0 || j > 0 {
            match traceback[i][j] {
                TracebackOp::Match => {
                    if query[i - 1] == target[j - 1] {
                        matches += 1;
                    } else {
                        mismatches += 1;
                    }
                    i -= 1;
                    j -= 1;
                }
                TracebackOp::Insert => {
                    insertions += 1;
                    i -= 1;
                }
                TracebackOp::Delete => {
                    deletions += 1;
                    j -= 1;
                }
                TracebackOp::None => break,
            }
        }
        
        let identity = if matches + mismatches + insertions + deletions > 0 {
            (matches as f32 / (matches + mismatches + insertions + deletions) as f32) * 100.0
        } else {
            0.0
        };
        
        Ok(AlignmentResult {
            identity,
            matches,
            mismatches,
            insertions,
            deletions,
            score: best_score,
            query_start: query_offset + i as u64,
            query_end: query_offset + end_i as u64,
            target_start: target_offset + j as u64,
            target_end: target_offset + end_j as u64,
        })
    }
    
    /// Verify multiple anchors in batch for efficiency
    pub fn verify_batch(
        &self,
        anchors: &[Anchor],
        query_seq: &[u8],
        target_seq: &[u8],
    ) -> Vec<VerifyResult<AlignmentResult>> {
        anchors
            .iter()
            .map(|anchor| self.verify_anchor(anchor, query_seq, target_seq))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TracebackOp {
    None,
    Match,
    Insert,
    Delete,
}

/// Reverse complement a DNA sequence
fn reverse_complement(seq: &[u8]) -> Vec<u8> {
    seq.iter()
        .rev()
        .map(|&base| match base.to_ascii_uppercase() {
            b'A' => b'T',
            b'T' => b'A',
            b'C' => b'G',
            b'G' => b'C',
            b'N' => b'N',
            _ => base, // Keep unknown bases as-is
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_perfect_match() {
        let query = b"ACGTACGTACGT";
        let target = b"ACGTACGTACGT";
        
        let anchor = Anchor::new(
            "query".to_string(),
            "target".to_string(),
            0, 12,
            0, 12,
            Strand::Forward,
            "test".to_string(),
        );
        
        let verifier = Verifier::new(VerifyParams::default());
        let result = verifier.verify_anchor(&anchor, query, target).unwrap();
        
        assert!(result.identity > 90.0);
        assert!(result.matches > 0);
        assert_eq!(result.mismatches, 0);
    }
    
    #[test]
    fn test_with_mismatches() {
        let query = b"ACGTACGTACGT";
        let target = b"ACGTACCGACGT"; // One mismatch
        
        let anchor = Anchor::new(
            "query".to_string(),
            "target".to_string(),
            0, 12,
            0, 12,
            Strand::Forward,
            "test".to_string(),
        );
        
        let verifier = Verifier::new(VerifyParams::default());
        let result = verifier.verify_anchor(&anchor, query, target).unwrap();
        
        assert!(result.identity < 100.0);
        assert!(result.identity > 80.0);
        assert!(result.mismatches > 0);
    }
    
    #[test]
    fn test_reverse_complement() {
        let seq = b"ACGT";
        let rc = reverse_complement(seq);
        assert_eq!(rc, b"ACGT"); // Palindrome
        
        let seq2 = b"AAAA";
        let rc2 = reverse_complement(seq2);
        assert_eq!(rc2, b"TTTT");
    }
    
    #[test]
    fn test_batch_verification() {
        let query = b"ACGTACGTACGT";
        let target = b"ACGTACGTACGT";
        
        let anchors = vec![
            Anchor::new(
                "query".to_string(),
                "target".to_string(),
                0, 6,
                0, 6,
                Strand::Forward,
                "test".to_string(),
            ),
            Anchor::new(
                "query".to_string(),
                "target".to_string(),
                6, 12,
                6, 12,
                Strand::Forward,
                "test".to_string(),
            ),
        ];
        
        let verifier = Verifier::new(VerifyParams::default());
        let results = verifier.verify_batch(&anchors, query, target);
        
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }
}