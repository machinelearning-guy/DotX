//! Core data types for DOTx
//!
//! This module contains the unified data structures for DOTx, including the
//! core Anchor model used across all components.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Position type for genomic coordinates
pub type Position = u64;

/// Strand orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Strand {
    Forward,
    Reverse,
}

impl std::fmt::Display for Strand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Strand::Forward => write!(f, "+"),
            Strand::Reverse => write!(f, "-"),
        }
    }
}

impl From<char> for Strand {
    fn from(c: char) -> Self {
        match c {
            '+' => Strand::Forward,
            '-' => Strand::Reverse,
            _ => Strand::Forward,
        }
    }
}

/// Sequence data structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sequence {
    pub id: String,
    pub description: Option<String>,
    pub data: Vec<u8>,
    pub length: Position,
}

impl Sequence {
    pub fn new(id: String, data: Vec<u8>) -> Self {
        let length = data.len() as Position;
        Self {
            id,
            description: None,
            data,
            length,
        }
    }
    
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}

/// Unified Anchor model as specified in the plan
/// Core data structure representing a seed-level match between query and target sequences
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    /// Query contig name
    pub q: String,
    /// Target contig name  
    pub t: String,
    /// Query start position (0-based)
    pub qs: u64,
    /// Query end position (exclusive)
    pub qe: u64,
    /// Target start position (0-based)
    pub ts: u64,
    /// Target end position (exclusive)
    pub te: u64,
    /// Strand orientation (+ or -)
    pub strand: Strand,
    /// Mapping quality (optional)
    pub mapq: Option<u8>,
    /// Identity percentage (optional, populated by verification)
    pub identity: Option<f32>,
    /// Which engine created this anchor
    pub engine_tag: String,
    
    // Extended metadata for compatibility with file formats
    pub query_length: Option<u64>,          // total query sequence length
    pub target_length: Option<u64>,         // total target sequence length
    pub residue_matches: Option<u32>,       // number of matching bases
    pub alignment_block_length: Option<u64>, // length of alignment block
    pub tags: HashMap<String, String>,      // additional tags from formats
}

impl Anchor {
    /// Create a basic anchor with core fields
    pub fn new(
        q: String,
        t: String,
        qs: u64,
        qe: u64,
        ts: u64,
        te: u64,
        strand: Strand,
        engine_tag: String,
    ) -> Self {
        Self {
            q,
            t,
            qs,
            qe,
            ts,
            te,
            strand,
            mapq: None,
            identity: None,
            engine_tag,
            query_length: None,
            target_length: None,
            residue_matches: None,
            alignment_block_length: None,
            tags: HashMap::new(),
        }
    }

    /// Create an Anchor from file parser data
    pub fn from_parser(
        query_name: String,
        query_length: u64,
        query_start: u64,
        query_end: u64,
        strand: Strand,
        target_name: String,
        target_length: u64,
        target_start: u64,
        target_end: u64,
        residue_matches: u32,
        alignment_block_length: u64,
    ) -> Self {
        Self {
            q: query_name,
            t: target_name,
            qs: query_start,
            qe: query_end,
            ts: target_start,
            te: target_end,
            strand,
            mapq: None,
            identity: None,
            engine_tag: "parser".to_string(),
            query_length: Some(query_length),
            target_length: Some(target_length),
            residue_matches: Some(residue_matches),
            alignment_block_length: Some(alignment_block_length),
            tags: HashMap::new(),
        }
    }

    pub fn with_mapping_quality(mut self, quality: u8) -> Self {
        self.mapq = Some(quality);
        self
    }

    pub fn with_identity(mut self, identity: f32) -> Self {
        self.identity = Some(identity);
        self
    }

    pub fn with_tag(mut self, key: String, value: String) -> Self {
        self.tags.insert(key, value);
        self
    }
    
    /// Query span length
    pub fn query_len(&self) -> u64 {
        self.qe - self.qs
    }
    
    /// Target span length  
    pub fn target_len(&self) -> u64 {
        self.te - self.ts
    }
    
    /// Average span length
    pub fn avg_len(&self) -> f64 {
        (self.query_len() + self.target_len()) as f64 / 2.0
    }
    
    /// Minimum span length
    pub fn min_len(&self) -> u64 {
        self.query_len().min(self.target_len())
    }

    /// Get identity as percentage (0.0 to 100.0)
    pub fn get_identity(&self) -> f64 {
        if let (Some(matches), Some(block_len)) = (self.residue_matches, self.alignment_block_length) {
            if block_len > 0 {
                (matches as f64 / block_len as f64) * 100.0
            } else {
                0.0
            }
        } else if let Some(id) = self.identity {
            id as f64
        } else {
            0.0
        }
    }

    /// Get identity as fraction (0.0 to 1.0)
    pub fn get_identity_fraction(&self) -> f64 {
        self.get_identity() / 100.0
    }

    /// Check if this anchor represents a diagonal match (same strand, similar coordinates)
    pub fn is_diagonal(&self) -> bool {
        self.strand == Strand::Forward &&
        (self.qs as i64 - self.ts as i64).abs() < 1000 // Allow some tolerance
    }

    /// Check if this anchor represents an anti-diagonal match (reverse strand)
    pub fn is_anti_diagonal(&self) -> bool {
        self.strand == Strand::Reverse
    }

    // Compatibility aliases for existing code
    pub fn query_name(&self) -> &str {
        &self.q
    }

    pub fn target_name(&self) -> &str {
        &self.t
    }

    pub fn query_start(&self) -> u64 {
        self.qs
    }

    pub fn query_end(&self) -> u64 {
        self.qe
    }

    pub fn target_start(&self) -> u64 {
        self.ts
    }

    pub fn target_end(&self) -> u64 {
        self.te
    }

    pub fn query_span_length(&self) -> u64 {
        self.query_len()
    }
    
    pub fn target_span_length(&self) -> u64 {
        self.target_len()
    }
    
    pub fn alignment_length(&self) -> u64 {
        self.min_len()
    }
}