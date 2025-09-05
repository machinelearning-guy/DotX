use serde::{Deserialize, Serialize};
use std::fmt;

pub type Position = u64;
pub type SequenceId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Coordinate {
    pub x: Position,
    pub y: Position,
}

impl Coordinate {
    pub fn new(x: Position, y: Position) -> Self {
        Self { x, y }
    }
    
    pub fn distance(&self, other: &Coordinate) -> f64 {
        let dx = (self.x as i64 - other.x as i64) as f64;
        let dy = (self.y as i64 - other.y as i64) as f64;
        (dx * dx + dy * dy).sqrt()
    }
}

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
    
    pub fn reverse_complement(&self) -> Self {
        let rev_data: Vec<u8> = self.data
            .iter()
            .rev()
            .map(|&base| match base {
                b'A' | b'a' => b'T',
                b'T' | b't' => b'A',
                b'G' | b'g' => b'C',
                b'C' | b'c' => b'G',
                _ => base,
            })
            .collect();
            
        Self {
            id: format!("{}_RC", self.id),
            description: self.description.as_ref().map(|d| format!("{} (reverse complement)", d)),
            data: rev_data,
            length: self.length,
        }
    }
    
    pub fn get_kmer(&self, start: Position, k: usize) -> Option<&[u8]> {
        let start = start as usize;
        if start + k <= self.data.len() {
            Some(&self.data[start..start + k])
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Strand {
    Forward,
    Reverse,
}

impl fmt::Display for Strand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Strand::Forward => write!(f, "+"),
            Strand::Reverse => write!(f, "-"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Match {
    pub x_start: Position,
    pub x_end: Position,
    pub y_start: Position,
    pub y_end: Position,
    pub strand: Strand,
    pub score: u32,
}

impl Match {
    pub fn new(x_start: Position, x_end: Position, y_start: Position, y_end: Position, strand: Strand) -> Self {
        let length = (x_end - x_start).min(y_end - y_start);
        Self {
            x_start,
            x_end,
            y_start,
            y_end,
            strand,
            score: length as u32,
        }
    }
    
    pub fn length(&self) -> Position {
        (self.x_end - self.x_start).min(self.y_end - self.y_start)
    }
    
    pub fn diagonal(&self) -> i64 {
        self.x_start as i64 - self.y_start as i64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotPlot {
    pub x_sequence: Sequence,
    pub y_sequence: Sequence,
    pub matches: Vec<Match>,
    pub k: usize,
    pub min_match_length: Position,
}

impl DotPlot {
    pub fn new(x_sequence: Sequence, y_sequence: Sequence, k: usize, min_match_length: Position) -> Self {
        Self {
            x_sequence,
            y_sequence,
            matches: Vec::new(),
            k,
            min_match_length,
        }
    }
    
    pub fn add_match(&mut self, m: Match) {
        if m.length() >= self.min_match_length {
            self.matches.push(m);
        }
    }
    
    pub fn total_matches(&self) -> usize {
        self.matches.len()
    }
}

// DOTx Phase 2 Core Data Structures

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Engine {
    Minimap2,
    Syncmer,
    Strobemer,
}

impl fmt::Display for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Engine::Minimap2 => write!(f, "minimap2"),
            Engine::Syncmer => write!(f, "syncmer"),
            Engine::Strobemer => write!(f, "strobemer"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub q: String,                           // query contig name
    pub t: String,                           // target contig name  
    pub qs: u64,                            // query start
    pub qe: u64,                            // query end
    pub ts: u64,                            // target start
    pub te: u64,                            // target end
    pub strand: Strand,                     // + or -
    pub mapq: Option<u8>,                   // mapping quality
    pub identity: Option<f32>,              // identity percentage
    pub engine_tag: String,                 // which engine created this
    
    // Extended fields for file parsers
    pub query_length: Option<u64>,          // total query sequence length
    pub target_length: Option<u64>,         // total target sequence length
    pub residue_matches: Option<u32>,       // number of matching bases
    pub alignment_block_length: Option<u64>, // length of alignment block
    pub tags: std::collections::HashMap<String, String>, // additional tags
}

impl Anchor {
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
            tags: std::collections::HashMap::new(),
        }
    }

    /// Create an Anchor from file parser data (for compatibility with parsers)
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
            tags: std::collections::HashMap::new(),
        }
    }
    
    pub fn with_mapq(mut self, mapq: u8) -> Self {
        self.mapq = Some(mapq);
        self
    }
    
    pub fn with_identity(mut self, identity: f32) -> Self {
        self.identity = Some(identity);
        self
    }

    pub fn with_mapping_quality(mut self, quality: u8) -> Self {
        self.mapq = Some(quality);
        self
    }

    pub fn with_tag(mut self, key: String, value: String) -> Self {
        self.tags.insert(key, value);
        self
    }
    
    pub fn query_span_length(&self) -> u64 {
        self.qe - self.qs
    }
    
    pub fn target_span_length(&self) -> u64 {
        self.te - self.ts
    }
    
    pub fn alignment_length(&self) -> u64 {
        self.query_span_length().min(self.target_span_length())
    }

    pub fn total_query_length(&self) -> Option<u64> {
        self.query_length
    }

    pub fn total_target_length(&self) -> Option<u64> {
        self.target_length
    }

    pub fn get_identity(&self) -> f64 {
        if let (Some(matches), Some(block_len)) = (self.residue_matches, self.alignment_block_length) {
            if block_len > 0 {
                matches as f64 / block_len as f64
            } else {
                0.0
            }
        } else if let Some(id) = self.identity {
            id as f64 / 100.0 // Convert percentage to fraction
        } else {
            0.0
        }
    }

    // Compatibility aliases for existing field names
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chain {
    pub anchors: Vec<Anchor>,
    pub score: f64,
    pub chain_id: u64,
}

impl Chain {
    pub fn new(chain_id: u64) -> Self {
        Self {
            anchors: Vec::new(),
            score: 0.0,
            chain_id,
        }
    }
    
    pub fn add_anchor(&mut self, anchor: Anchor) {
        self.anchors.push(anchor);
        self.update_score();
    }
    
    pub fn len(&self) -> usize {
        self.anchors.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
    }
    
    pub fn total_query_span(&self) -> u64 {
        if self.anchors.is_empty() {
            return 0;
        }
        let min_qs = self.anchors.iter().map(|a| a.qs).min().unwrap();
        let max_qe = self.anchors.iter().map(|a| a.qe).max().unwrap();
        max_qe - min_qs
    }
    
    pub fn total_target_span(&self) -> u64 {
        if self.anchors.is_empty() {
            return 0;
        }
        let min_ts = self.anchors.iter().map(|a| a.ts).min().unwrap();
        let max_te = self.anchors.iter().map(|a| a.te).max().unwrap();
        max_te - min_ts
    }
    
    fn update_score(&mut self) {
        self.score = self.anchors.iter()
            .map(|a| a.alignment_length() as f64)
            .sum();
    }
}