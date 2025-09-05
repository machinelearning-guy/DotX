//! MAF (Multiple Alignment Format) file parser
//!
//! MAF is a text format for storing multiple alignments at the DNA level.
//! Each alignment block begins with an "a" line and contains one or more "s" lines.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use anyhow::{anyhow, Result};
use thiserror::Error;
use flate2::read::GzDecoder;

use crate::types::{Anchor, Position, Strand};

#[derive(Debug, Error)]
pub enum MafError {
    #[error("Invalid MAF line format: {0}")]
    InvalidFormat(String),
    #[error("Missing alignment block")]
    MissingAlignmentBlock,
    #[error("Invalid position value: {0}")]
    InvalidPosition(String),
    #[error("Invalid strand: {0}")]
    InvalidStrand(String),
    #[error("Invalid sequence line: {0}")]
    InvalidSequenceLine(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Represents a sequence within a MAF alignment block
#[derive(Debug, Clone)]
pub struct MafSequence {
    pub src: String,        // Source sequence name
    pub start: Position,    // Start position in source
    pub size: Position,     // Size of aligned sequence
    pub strand: Strand,     // Strand orientation
    pub src_size: Position, // Total size of source sequence
    pub text: String,       // Aligned sequence with gaps
}

/// Represents a MAF alignment block
#[derive(Debug, Clone)]
pub struct MafBlock {
    pub score: Option<f64>,
    pub sequences: Vec<MafSequence>,
    pub metadata: HashMap<String, String>,
}

/// MAF parser for reading multiple alignment blocks
pub struct MafParser;

impl MafParser {
    /// Parse a MAF file and return a vector of Anchors
    /// Each pairwise alignment within multi-way blocks becomes an Anchor
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Vec<Anchor>> {
        let file = File::open(&path)?;
        let path_str = path.as_ref().to_string_lossy();
        
        if path_str.ends_with(".gz") {
            let decoder = GzDecoder::new(file);
            let reader = BufReader::new(decoder);
            Self::parse_reader(reader)
        } else {
            let reader = BufReader::new(file);
            Self::parse_reader(reader)
        }
    }

    /// Parse MAF data from any BufRead source
    pub fn parse_reader<R: BufRead>(reader: R) -> Result<Vec<Anchor>> {
        let blocks = Self::parse_blocks(reader)?;
        let mut anchors = Vec::new();
        
        for block in blocks {
            // Convert each alignment block to pairwise anchors
            let block_anchors = Self::block_to_anchors(block)?;
            anchors.extend(block_anchors);
        }
        
        Ok(anchors)
    }

    /// Parse MAF blocks from a reader
    pub fn parse_blocks<R: BufRead>(reader: R) -> Result<Vec<MafBlock>> {
        let mut blocks = Vec::new();
        let mut current_block: Option<MafBlock> = None;
        let mut line_number = 0;

        for line in reader.lines() {
            line_number += 1;
            let line = line?;
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(first_char) = trimmed.chars().next() {
                match first_char {
                    'a' => {
                        // Start of new alignment block
                        if let Some(block) = current_block.take() {
                            blocks.push(block);
                        }
                        current_block = Some(Self::parse_alignment_line(trimmed)?);
                    }
                    's' => {
                        // Sequence line
                        if let Some(ref mut block) = current_block {
                            let sequence = Self::parse_sequence_line(trimmed)
                                .map_err(|e| anyhow!("Error parsing sequence line {}: {}", line_number, e))?;
                            block.sequences.push(sequence);
                        } else {
                            return Err(anyhow!("Sequence line without alignment block at line {}", line_number));
                        }
                    }
                    'i' | 'e' | 'q' => {
                        // Optional lines (insert, empty, quality) - store as metadata
                        if let Some(ref mut block) = current_block {
                            let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
                            if parts.len() == 2 {
                                block.metadata.insert(
                                    format!("{}_{}", first_char, block.metadata.len()),
                                    parts[1].to_string()
                                );
                            }
                        }
                    }
                    _ => {
                        // Unknown line type - ignore or store as metadata
                        if let Some(ref mut block) = current_block {
                            block.metadata.insert(
                                format!("unknown_{}", block.metadata.len()),
                                trimmed.to_string()
                            );
                        }
                    }
                }
            }
        }

        // Add the last block if exists
        if let Some(block) = current_block {
            blocks.push(block);
        }

        Ok(blocks)
    }

    /// Parse an alignment line (starts with 'a')
    fn parse_alignment_line(line: &str) -> Result<MafBlock, MafError> {
        let mut metadata = HashMap::new();
        let mut score = None;

        // Parse the alignment line: "a score=12345 ..."
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        for part in parts.iter().skip(1) { // Skip the 'a'
            if let Some((key, value)) = part.split_once('=') {
                if key == "score" {
                    score = value.parse::<f64>().ok();
                } else {
                    metadata.insert(key.to_string(), value.to_string());
                }
            }
        }

        Ok(MafBlock {
            score,
            sequences: Vec::new(),
            metadata,
        })
    }

    /// Parse a sequence line (starts with 's')
    fn parse_sequence_line(line: &str) -> Result<MafSequence, MafError> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() < 7 {
            return Err(MafError::InvalidSequenceLine(format!(
                "Expected at least 7 fields, got {}", parts.len()
            )));
        }

        let src = parts[1].to_string();
        let start = parts[2].parse::<Position>()
            .map_err(|_| MafError::InvalidPosition(parts[2].to_string()))?;
        let size = parts[3].parse::<Position>()
            .map_err(|_| MafError::InvalidPosition(parts[3].to_string()))?;
        
        let strand = match parts[4] {
            "+" => Strand::Forward,
            "-" => Strand::Reverse,
            s => return Err(MafError::InvalidStrand(s.to_string())),
        };
        
        let src_size = parts[5].parse::<Position>()
            .map_err(|_| MafError::InvalidPosition(parts[5].to_string()))?;
        
        let text = parts[6].to_string();

        Ok(MafSequence {
            src,
            start,
            size,
            strand,
            src_size,
            text,
        })
    }

    /// Convert a MAF block to pairwise anchors
    /// For multi-way alignments, creates anchors between the first sequence and all others
    fn block_to_anchors(block: MafBlock) -> Result<Vec<Anchor>> {
        let mut anchors = Vec::new();
        
        if block.sequences.len() < 2 {
            return Ok(anchors); // Need at least 2 sequences for an anchor
        }

        let reference = &block.sequences[0];
        
        for target in block.sequences.iter().skip(1) {
            let anchor = Self::sequences_to_anchor(reference, target, &block)?;
            anchors.push(anchor);
        }

        Ok(anchors)
    }

    /// Convert two MAF sequences to an Anchor
    fn sequences_to_anchor(
        reference: &MafSequence,
        target: &MafSequence,
        block: &MafBlock,
    ) -> Result<Anchor> {
        // Calculate residue matches by comparing aligned sequences
        let residue_matches = Self::count_matches(&reference.text, &target.text);
        
        // For MAF, the alignment block length is the length of the gapped alignment
        let alignment_block_length = reference.text.len() as Position;

        let mut anchor = Anchor::from_parser(
            target.src.clone(),
            target.src_size,
            target.start,
            target.start + target.size,
            target.strand,
            reference.src.clone(),
            reference.src_size,
            reference.start,
            reference.start + reference.size,
            residue_matches,
            alignment_block_length,
        );

        // Add score as a tag if available
        if let Some(score) = block.score {
            anchor = anchor.with_tag("score".to_string(), score.to_string());
        }

        // Add other metadata as tags
        for (key, value) in &block.metadata {
            anchor = anchor.with_tag(key.clone(), value.clone());
        }

        Ok(anchor)
    }

    /// Count matching residues between two aligned sequences
    fn count_matches(seq1: &str, seq2: &str) -> u32 {
        seq1.chars()
            .zip(seq2.chars())
            .map(|(c1, c2)| {
                if c1 != '-' && c2 != '-' && c1.to_ascii_uppercase() == c2.to_ascii_uppercase() {
                    1
                } else {
                    0
                }
            })
            .sum()
    }

    /// Create an iterator over MAF blocks
    pub fn iter_blocks<P: AsRef<Path>>(path: P) -> Result<MafBlockIterator> {
        let file = File::open(&path)?;
        let path_str = path.as_ref().to_string_lossy();
        
        if path_str.ends_with(".gz") {
            let decoder = GzDecoder::new(file);
            let reader = BufReader::new(decoder);
            Ok(MafBlockIterator::new(reader))
        } else {
            let reader = BufReader::new(file);
            Ok(MafBlockIterator::new(reader))
        }
    }

    /// Create an iterator over anchors from a MAF file
    pub fn iter_anchors<P: AsRef<Path>>(path: P) -> Result<MafAnchorIterator> {
        Ok(MafAnchorIterator::new(Self::iter_blocks(path)?))
    }
}

/// Iterator over MAF blocks
pub struct MafBlockIterator<R: BufRead> {
    reader: R,
    line_buffer: String,
    line_number: usize,
    current_block: Option<MafBlock>,
}

impl<R: BufRead> MafBlockIterator<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            line_buffer: String::new(),
            line_number: 0,
            current_block: None,
        }
    }
}

impl<R: BufRead> Iterator for MafBlockIterator<R> {
    type Item = Result<MafBlock>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.line_buffer.clear();
            
            match self.reader.read_line(&mut self.line_buffer) {
                Ok(0) => {
                    // EOF - return current block if exists
                    return self.current_block.take().map(Ok);
                }
                Ok(_) => {
                    self.line_number += 1;
                    let line = self.line_buffer.trim();
                    
                    // Skip empty lines and comments
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }

                    if let Some(first_char) = line.chars().next() {
                        match first_char {
                            'a' => {
                                // Start of new alignment block
                                let previous_block = self.current_block.take();
                                
                                match MafParser::parse_alignment_line(line) {
                                    Ok(block) => {
                                        self.current_block = Some(block);
                                        if let Some(block) = previous_block {
                                            return Some(Ok(block));
                                        }
                                    }
                                    Err(e) => return Some(Err(e.into())),
                                }
                            }
                            's' => {
                                // Sequence line
                                if let Some(ref mut block) = self.current_block {
                                    match MafParser::parse_sequence_line(line) {
                                        Ok(sequence) => block.sequences.push(sequence),
                                        Err(e) => return Some(Err(anyhow!(
                                            "Error parsing sequence line {}: {}", self.line_number, e
                                        ))),
                                    }
                                } else {
                                    return Some(Err(anyhow!(
                                        "Sequence line without alignment block at line {}", self.line_number
                                    )));
                                }
                            }
                            'i' | 'e' | 'q' => {
                                // Optional lines - store as metadata
                                if let Some(ref mut block) = self.current_block {
                                    let parts: Vec<&str> = line.splitn(2, ' ').collect();
                                    if parts.len() == 2 {
                                        block.metadata.insert(
                                            format!("{}_{}", first_char, block.metadata.len()),
                                            parts[1].to_string()
                                        );
                                    }
                                }
                            }
                            _ => {
                                // Unknown line type - store as metadata
                                if let Some(ref mut block) = self.current_block {
                                    block.metadata.insert(
                                        format!("unknown_{}", block.metadata.len()),
                                        line.to_string()
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => return Some(Err(e.into())),
            }
        }
    }
}

/// Iterator over anchors from MAF blocks
pub struct MafAnchorIterator {
    block_iterator: MafBlockIterator<BufReader<Box<dyn std::io::Read>>>,
    current_anchors: std::vec::IntoIter<Anchor>,
}

impl MafAnchorIterator {
    fn new(block_iterator: MafBlockIterator<BufReader<Box<dyn std::io::Read>>>) -> Self {
        Self {
            block_iterator,
            current_anchors: Vec::new().into_iter(),
        }
    }
}

impl Iterator for MafAnchorIterator {
    type Item = Result<Anchor>;

    fn next(&mut self) -> Option<Self::Item> {
        // First, try to get an anchor from current block
        if let Some(anchor) = self.current_anchors.next() {
            return Some(Ok(anchor));
        }

        // If no more anchors in current block, get next block
        match self.block_iterator.next() {
            Some(Ok(block)) => {
                match MafParser::block_to_anchors(block) {
                    Ok(anchors) => {
                        self.current_anchors = anchors.into_iter();
                        self.current_anchors.next().map(Ok)
                    }
                    Err(e) => Some(Err(e)),
                }
            }
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_alignment_line() {
        let line = "a score=12345 pass=2";
        let block = MafParser::parse_alignment_line(line).unwrap();
        
        assert_eq!(block.score, Some(12345.0));
        assert_eq!(block.metadata.get("pass"), Some(&"2".to_string()));
        assert!(block.sequences.is_empty());
    }

    #[test]
    fn test_parse_sequence_line() {
        let line = "s hg18.chr7    27707221 13 + 158545518 gcagctgaaaaca";
        let seq = MafParser::parse_sequence_line(line).unwrap();
        
        assert_eq!(seq.src, "hg18.chr7");
        assert_eq!(seq.start, 27707221);
        assert_eq!(seq.size, 13);
        assert_eq!(seq.strand, Strand::Forward);
        assert_eq!(seq.src_size, 158545518);
        assert_eq!(seq.text, "gcagctgaaaaca");
    }

    #[test]
    fn test_parse_reverse_strand() {
        let line = "s panTro1.chr6 28869787 13 - 161576975 gcagctgaaaaca";
        let seq = MafParser::parse_sequence_line(line).unwrap();
        
        assert_eq!(seq.strand, Strand::Reverse);
    }

    #[test]
    fn test_count_matches() {
        let seq1 = "ATCG-N";
        let seq2 = "ATCG-A";
        let matches = MafParser::count_matches(seq1, seq2);
        assert_eq!(matches, 4); // ATCG matches, - matches -, N != A
    }

    #[test]
    fn test_parse_maf_block() {
        let maf_data = "a score=12345\n\
                        s hg18.chr7    27707221 13 + 158545518 gcagctgaaaaca\n\
                        s panTro1.chr6 28869787 13 - 161576975 gcagctgaaaaca\n";
        
        let cursor = Cursor::new(maf_data);
        let blocks = MafParser::parse_blocks(cursor).unwrap();
        
        assert_eq!(blocks.len(), 1);
        let block = &blocks[0];
        assert_eq!(block.score, Some(12345.0));
        assert_eq!(block.sequences.len(), 2);
        assert_eq!(block.sequences[0].src, "hg18.chr7");
        assert_eq!(block.sequences[1].src, "panTro1.chr6");
    }

    #[test]
    fn test_block_to_anchors() {
        let block = MafBlock {
            score: Some(12345.0),
            sequences: vec![
                MafSequence {
                    src: "hg18.chr7".to_string(),
                    start: 27707221,
                    size: 13,
                    strand: Strand::Forward,
                    src_size: 158545518,
                    text: "gcagctgaaaaca".to_string(),
                },
                MafSequence {
                    src: "panTro1.chr6".to_string(),
                    start: 28869787,
                    size: 13,
                    strand: Strand::Reverse,
                    src_size: 161576975,
                    text: "gcagctgaaaaca".to_string(),
                },
            ],
            metadata: HashMap::new(),
        };
        
        let anchors = MafParser::block_to_anchors(block).unwrap();
        
        assert_eq!(anchors.len(), 1);
        let anchor = &anchors[0];
        assert_eq!(anchor.query_name, "panTro1.chr6");
        assert_eq!(anchor.target_name, "hg18.chr7");
        assert_eq!(anchor.residue_matches, 13);
        assert_eq!(anchor.tags.get("score"), Some(&"12345".to_string()));
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let maf_data = "a score=12345\n\
                        s hg18.chr7    27707221 13 + 158545518 gcagctgaaaaca\n\
                        s panTro1.chr6 28869787 13 - 161576975 gcagctgaaaaca\n\
                        \n\
                        a score=67890\n\
                        s hg18.chr7    27707250 10 + 158545518 gcagctgaaa\n\
                        s panTro1.chr6 28869800 10 + 161576975 gcagctgaaa\n";
        
        let cursor = Cursor::new(maf_data);
        let anchors = MafParser::parse_reader(cursor).unwrap();
        
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].tags.get("score"), Some(&"12345".to_string()));
        assert_eq!(anchors[1].tags.get("score"), Some(&"67890".to_string()));
        assert_eq!(anchors[1].strand, Strand::Forward);
    }
}