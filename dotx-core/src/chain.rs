//! Chaining module for DOTx
//!
//! Implements concave-gap dynamic programming for chaining seed anchors
//! into alignment paths, following the minimap2-style approach.

use crate::types::{Anchor, Strand};
use std::cmp::Ordering;
use thiserror::Error;

/// Errors that can occur during chaining
#[derive(Debug, Error)]
pub enum ChainError {
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
    
    #[error("No anchors provided")]
    NoAnchors,
    
    #[error("Memory allocation failed")]
    OutOfMemory,
}

pub type ChainResult<T> = Result<T, ChainError>;

/// Parameters for the chaining algorithm
#[derive(Debug, Clone)]
pub struct ChainParams {
    /// Maximum gap penalty for concave gap cost
    pub max_gap: f32,
    /// Gap extension penalty
    pub gap_extend: f32,
    /// Minimum chain score to keep
    pub min_score: f32,
    /// Maximum number of chains to keep per query-target pair
    pub max_chains: usize,
    /// Maximum linking distance (in bp)
    pub max_distance: u64,
    /// Bandwidth for diagonal filtering
    pub bandwidth: i64,
}

impl Default for ChainParams {
    fn default() -> Self {
        Self {
            max_gap: 5000.0,
            gap_extend: 0.01,
            min_score: 40.0,
            max_chains: 50,
            max_distance: 100_000,
            bandwidth: 500,
        }
    }
}

/// A chain of anchors representing an alignment path
#[derive(Debug, Clone)]
pub struct Chain {
    pub id: u32,
    pub query_contig: String,
    pub target_contig: String,
    pub anchors: Vec<usize>, // Indices into the original anchor array
    pub score: f32,
    pub strand: Strand,
}

impl Chain {
    pub fn new(
        id: u32,
        query_contig: String,
        target_contig: String,
        strand: Strand,
    ) -> Self {
        Self {
            id,
            query_contig,
            target_contig,
            anchors: Vec::new(),
            score: 0.0,
            strand,
        }
    }
    
    /// Add an anchor to the chain
    pub fn add_anchor(&mut self, anchor_idx: usize, score_increase: f32) {
        self.anchors.push(anchor_idx);
        self.score += score_increase;
    }
    
    /// Get the span of this chain in query coordinates
    pub fn query_span(&self, anchors: &[Anchor]) -> (u64, u64) {
        if self.anchors.is_empty() {
            return (0, 0);
        }
        
        let mut min_start = u64::MAX;
        let mut max_end = 0u64;
        
        for &idx in &self.anchors {
            if let Some(anchor) = anchors.get(idx) {
                min_start = min_start.min(anchor.qs);
                max_end = max_end.max(anchor.qe);
            }
        }
        
        (min_start, max_end)
    }
    
    /// Get the span of this chain in target coordinates
    pub fn target_span(&self, anchors: &[Anchor]) -> (u64, u64) {
        if self.anchors.is_empty() {
            return (0, 0);
        }
        
        let mut min_start = u64::MAX;
        let mut max_end = 0u64;
        
        for &idx in &self.anchors {
            if let Some(anchor) = anchors.get(idx) {
                min_start = min_start.min(anchor.ts);
                max_end = max_end.max(anchor.te);
            }
        }
        
        (min_start, max_end)
    }
}

/// Chaining algorithm implementation
pub struct Chainer {
    params: ChainParams,
}

impl Chainer {
    pub fn new(params: ChainParams) -> Self {
        Self { params }
    }
    
    /// Perform chaining on a set of anchors
    pub fn chain(&self, anchors: &[Anchor]) -> ChainResult<Vec<Chain>> {
        if anchors.is_empty() {
            return Err(ChainError::NoAnchors);
        }
        
        // Group anchors by query-target pair and strand
        let mut groups = std::collections::HashMap::new();
        
        for (i, anchor) in anchors.iter().enumerate() {
            let key = (anchor.q.clone(), anchor.t.clone(), anchor.strand);
            groups.entry(key).or_insert_with(Vec::new).push(i);
        }
        
        let mut all_chains = Vec::new();
        let mut chain_id = 0u32;
        
        let groups_len = groups.len();
        // Chain each group independently
        for ((query, target, strand), mut anchor_indices) in groups {
            // Sort anchors by diagonal (query + target position for consistency)
            anchor_indices.sort_by(|&a, &b| {
                let anchor_a = &anchors[a];
                let anchor_b = &anchors[b];
                
                let diag_a = anchor_a.qs as i64 + anchor_a.ts as i64;
                let diag_b = anchor_b.qs as i64 + anchor_b.ts as i64;
                
                diag_a.cmp(&diag_b)
                    .then(anchor_a.qs.cmp(&anchor_b.qs))
                    .then(anchor_a.ts.cmp(&anchor_b.ts))
            });
            
            let chains = self.chain_group(anchors, &anchor_indices, &query, &target, strand, &mut chain_id)?;
            all_chains.extend(chains);
        }
        
        // Sort chains by score and keep top ones
        all_chains.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        all_chains.truncate(self.params.max_chains * groups_len);
        
        // Filter by minimum score
        all_chains.retain(|chain| chain.score >= self.params.min_score);
        
        Ok(all_chains)
    }
    
    /// Chain a group of anchors from the same query-target pair
    fn chain_group(
        &self,
        anchors: &[Anchor],
        anchor_indices: &[usize],
        query: &str,
        target: &str,
        strand: Strand,
        chain_id: &mut u32,
    ) -> ChainResult<Vec<Chain>> {
        let n = anchor_indices.len();
        if n == 0 {
            return Ok(Vec::new());
        }
        
        // DP arrays
        let mut dp_score = vec![0.0f32; n];
        let mut dp_parent = vec![None::<usize>; n];
        
        // Initialize first anchor
        dp_score[0] = anchors[anchor_indices[0]].avg_len() as f32;
        
        // Fill DP table
        for i in 1..n {
            let anchor_i = &anchors[anchor_indices[i]];
            dp_score[i] = anchor_i.avg_len() as f32; // Score for singleton chain
            
            for j in 0..i {
                let anchor_j = &anchors[anchor_indices[j]];
                
                // Check if anchors can be linked
                if !self.can_link(anchor_i, anchor_j) {
                    continue;
                }
                
                // Calculate gap cost using concave gap penalty
                let gap_cost = self.gap_cost(anchor_i, anchor_j);
                let candidate_score = dp_score[j] + anchor_i.avg_len() as f32 - gap_cost;
                
                if candidate_score > dp_score[i] {
                    dp_score[i] = candidate_score;
                    dp_parent[i] = Some(j);
                }
            }
        }
        
        // Backtrack to find chains
        let mut chains = Vec::new();
        let mut used = vec![false; n];
        
        // Find chain endpoints (sorted by score)
        let mut endpoints: Vec<usize> = (0..n).collect();
        endpoints.sort_by(|&a, &b| dp_score[b].partial_cmp(&dp_score[a]).unwrap_or(Ordering::Equal));
        
        for &end in &endpoints {
            if used[end] || dp_score[end] < self.params.min_score {
                continue;
            }
            
            // Build chain by backtracking
            let mut chain = Chain::new(*chain_id, query.to_string(), target.to_string(), strand);
            *chain_id += 1;
            
            let mut current = Some(end);
            let mut chain_indices = Vec::new();
            
            while let Some(idx) = current {
                chain_indices.push(anchor_indices[idx]);
                used[idx] = true;
                current = dp_parent[idx];
            }
            
            // Reverse to get correct order
            chain_indices.reverse();
            chain.anchors = chain_indices;
            chain.score = dp_score[end];
            
            chains.push(chain);
            
            if chains.len() >= self.params.max_chains {
                break;
            }
        }
        
        Ok(chains)
    }
    
    /// Check if two anchors can be linked in a chain
    fn can_link(&self, anchor_i: &Anchor, anchor_j: &Anchor) -> bool {
        // Must be same query and target contigs
        if anchor_i.q != anchor_j.q || anchor_i.t != anchor_j.t {
            return false;
        }
        
        // Must be same strand
        if anchor_i.strand != anchor_j.strand {
            return false;
        }
        
        // Check coordinate ordering
        let q_forward = anchor_i.qs >= anchor_j.qe;
        let t_forward = anchor_i.ts >= anchor_j.te;
        
        match anchor_i.strand {
            Strand::Forward => {
                // Both coordinates should be forward
                if !q_forward || !t_forward {
                    return false;
                }
            }
            Strand::Reverse => {
                // Query forward, target can be either direction for reverse complement
                if !q_forward {
                    return false;
                }
            }
        }
        
        // Check distance constraints
        let q_dist = anchor_i.qs - anchor_j.qe;
        let t_dist = anchor_i.ts.max(anchor_j.te) - anchor_i.ts.min(anchor_j.te);
        
        if q_dist > self.params.max_distance || t_dist > self.params.max_distance {
            return false;
        }
        
        // Check diagonal bandwidth
        let diag_i = anchor_i.qs as i64 - anchor_i.ts as i64;
        let diag_j = anchor_j.qs as i64 - anchor_j.ts as i64;
        
        if (diag_i - diag_j).abs() > self.params.bandwidth {
            return false;
        }
        
        true
    }
    
    /// Calculate concave gap cost between two anchors
    fn gap_cost(&self, anchor_i: &Anchor, anchor_j: &Anchor) -> f32 {
        let q_gap = if anchor_i.qs > anchor_j.qe {
            anchor_i.qs - anchor_j.qe
        } else {
            0
        };
        
        let t_gap = if anchor_i.ts > anchor_j.te {
            anchor_i.ts - anchor_j.te
        } else if anchor_j.ts > anchor_i.te {
            anchor_j.ts - anchor_i.te
        } else {
            0
        };
        
        let gap = q_gap.max(t_gap) as f32;
        
        if gap == 0.0 {
            return 0.0;
        }
        
        // Concave gap cost: log-scale penalty to avoid over-penalizing large gaps
        let base_cost = self.params.gap_extend * gap;
        let concave_factor = (1.0 + gap / self.params.max_gap).ln();
        
        base_cost * concave_factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_chaining() {
        let anchors = vec![
            Anchor::new(
                "chr1".to_string(),
                "chr2".to_string(),
                1000, 1100,
                5000, 5100,
                Strand::Forward,
                "test".to_string(),
            ),
            Anchor::new(
                "chr1".to_string(),
                "chr2".to_string(),
                1200, 1300,
                5200, 5300,
                Strand::Forward,
                "test".to_string(),
            ),
            Anchor::new(
                "chr1".to_string(),
                "chr2".to_string(),
                1400, 1500,
                5400, 5500,
                Strand::Forward,
                "test".to_string(),
            ),
        ];
        
        let chainer = Chainer::new(ChainParams::default());
        let chains = chainer.chain(&anchors).unwrap();
        
        assert!(!chains.is_empty());
        assert_eq!(chains[0].anchors.len(), 3);
        assert!(chains[0].score > 0.0);
    }
    
    #[test]
    fn test_strand_separation() {
        let anchors = vec![
            Anchor::new(
                "chr1".to_string(),
                "chr2".to_string(),
                1000, 1100,
                5000, 5100,
                Strand::Forward,
                "test".to_string(),
            ),
            Anchor::new(
                "chr1".to_string(),
                "chr2".to_string(),
                1200, 1300,
                5200, 5300,
                Strand::Reverse,
                "test".to_string(),
            ),
        ];
        
        let chainer = Chainer::new(ChainParams::default());
        let chains = chainer.chain(&anchors).unwrap();
        
        // Should create separate chains for different strands
        assert_eq!(chains.len(), 2);
        assert_ne!(chains[0].strand, chains[1].strand);
    }
    
    #[test]
    fn test_gap_cost() {
        let chainer = Chainer::new(ChainParams::default());
        
        let anchor1 = Anchor::new(
            "chr1".to_string(),
            "chr2".to_string(),
            1000, 1100,
            5000, 5100,
            Strand::Forward,
            "test".to_string(),
        );
        
        let anchor2 = Anchor::new(
            "chr1".to_string(),
            "chr2".to_string(),
            1200, 1300,
            5200, 5300,
            Strand::Forward,
            "test".to_string(),
        );
        
        let gap_cost = chainer.gap_cost(&anchor2, &anchor1);
        assert!(gap_cost > 0.0);
        
        // Gap cost should increase with distance
        let anchor3 = Anchor::new(
            "chr1".to_string(),
            "chr2".to_string(),
            2000, 2100,
            6000, 6100,
            Strand::Forward,
            "test".to_string(),
        );
        
        let larger_gap_cost = chainer.gap_cost(&anchor3, &anchor1);
        assert!(larger_gap_cost > gap_cost);
    }
}
