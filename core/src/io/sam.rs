//! SAM/BAM file parser for extracting alignment coordinates
//!
//! Parses SAM (Sequence Alignment/Map) format files and converts them to Anchor structs.
//! Supports both text SAM and binary BAM formats using the noodles library.

use std::fs::File;
use std::path::Path;
use anyhow::{Result};
use thiserror::Error;

use noodles::sam::{self as sam, alignment::Record, header::Header};
use noodles::bam;
use noodles::bgzf;

use crate::types::{Anchor, Position, Strand};

#[derive(Debug, Error)]
pub enum SamError {
    #[error("Invalid CIGAR operation: {0}")]
    InvalidCigar(String),
    #[error("Missing reference name")]
    MissingReference,
    #[error("Invalid position: {0}")]
    InvalidPosition(i32),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("BAM parsing error: {0}")]
    Bam(String),
}

/// SAM/BAM parser for reading alignment records
pub struct SamParser;

impl SamParser {
    /// Parse a SAM file and return a vector of Anchors
    pub fn parse_sam_file<P: AsRef<Path>>(path: P) -> Result<Vec<Anchor>> {
        let mut reader = sam::io::reader::Builder::default()
            .build_from_path(path)?;
        
        let header = reader.read_header()?;
        Self::parse_sam_records(reader, &header)
    }

    /// Parse a BAM file and return a vector of Anchors
    pub fn parse_bam_file<P: AsRef<Path>>(path: P) -> Result<Vec<Anchor>> {
        let mut reader = File::open(path)
            .map(bgzf::Reader::new)
            .map(bam::io::Reader::new)?;

        let header = reader.read_header()?;
        Self::parse_bam_records(reader, &header)
    }

    /// Parse SAM records from a reader
    fn parse_sam_records<R: std::io::BufRead>(
        mut reader: sam::io::Reader<R>,
        header: &Header,
    ) -> Result<Vec<Anchor>> {
        let mut anchors = Vec::new();
        
        for result in reader.records(header) {
            let record = result?;
            if let Some(anchor) = Self::record_to_anchor(&record, header)? {
                anchors.push(anchor);
            }
        }
        
        Ok(anchors)
    }

    /// Parse BAM records from a reader
    fn parse_bam_records<R: std::io::Read>(
        mut reader: bam::io::Reader<bgzf::Reader<R>>,
        header: &Header,
    ) -> Result<Vec<Anchor>> {
        let mut anchors = Vec::new();
        
        for result in reader.records(header) {
            let record = result.map_err(|e| SamError::Bam(e.to_string()))?;
            if let Some(anchor) = Self::record_to_anchor(&record, header)? {
                anchors.push(anchor);
            }
        }
        
        Ok(anchors)
    }

    /// Convert a SAM/BAM record to an Anchor
    fn record_to_anchor(
        record: &dyn Record,
        header: &Header,
    ) -> Result<Option<Anchor>, SamError> {
        // Skip unmapped reads
        if record.flags().is_unmapped() {
            return Ok(None);
        }

        // Get reference sequence name
        let reference_sequence_id = record.reference_sequence_id()
            .ok_or(SamError::MissingReference)?;

        let reference_name = header
            .reference_sequences()
            .get_index(reference_sequence_id)
            .map(|(name, _)| name.to_string())
            .ok_or(SamError::MissingReference)?;

        // Get alignment position (1-based in SAM, convert to 0-based)
        let alignment_start = record.alignment_start()
            .map(|pos| pos.get() as Position - 1)
            .ok_or(SamError::InvalidPosition(-1))?;

        // Get query name and sequence length
        let query_name = record.name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let query_sequence = record.sequence();
        let query_length = query_sequence.len() as Position;

        // Determine strand
        let strand = if record.flags().is_reverse_complemented() {
            Strand::Reverse
        } else {
            Strand::Forward
        };

        // Parse CIGAR to determine alignment boundaries
        let cigar = record.cigar();
        let (query_start, query_end, alignment_end, residue_matches) = 
            Self::parse_cigar_boundaries(&cigar, alignment_start)?;

        // Get mapping quality
        let mapping_quality = record.mapping_quality()
            .map(|mq| mq.get());

        // Get reference sequence length from header
        let reference_length = header
            .reference_sequences()
            .get_index(reference_sequence_id)
            .and_then(|(_, reference_sequence)| reference_sequence.length())
            .map(|len| len.get() as Position)
            .unwrap_or(0);

        // Calculate alignment block length
        let alignment_block_length = alignment_end - alignment_start;

        let mut anchor = Anchor::from_parser(
            query_name,
            query_length,
            query_start,
            query_end,
            strand,
            reference_name,
            reference_length,
            alignment_start,
            alignment_end,
            residue_matches,
            alignment_block_length,
        );

        if let Some(quality) = mapping_quality {
            anchor = anchor.with_mapping_quality(quality);
        }

        Ok(Some(anchor))
    }

    /// Parse CIGAR string to determine alignment boundaries and matches
    fn parse_cigar_boundaries(
        cigar: &dyn sam::alignment::record::Cigar,
        alignment_start: Position,
    ) -> Result<(Position, Position, Position, u32), SamError> {
        let mut query_pos = 0u64;
        let mut reference_pos = alignment_start;
        let mut query_start = None;
        let mut residue_matches = 0u32;

        for op in cigar.iter() {
            let op = op.map_err(|e| SamError::InvalidCigar(e.to_string()))?;
            let len = op.len() as u64;

            match op.kind() {
                sam::alignment::record::cigar::op::Kind::Match
                | sam::alignment::record::cigar::op::Kind::SequenceMatch => {
                    if query_start.is_none() {
                        query_start = Some(query_pos);
                    }
                    query_pos += len;
                    reference_pos += len;
                    residue_matches += len as u32;
                }
                sam::alignment::record::cigar::op::Kind::Insertion
                | sam::alignment::record::cigar::op::Kind::SoftClip => {
                    query_pos += len;
                }
                sam::alignment::record::cigar::op::Kind::Deletion
                | sam::alignment::record::cigar::op::Kind::Skip => {
                    reference_pos += len;
                }
                sam::alignment::record::cigar::op::Kind::HardClip
                | sam::alignment::record::cigar::op::Kind::Pad => {
                    // These don't consume query or reference positions
                }
                sam::alignment::record::cigar::op::Kind::SequenceMismatch => {
                    if query_start.is_none() {
                        query_start = Some(query_pos);
                    }
                    query_pos += len;
                    reference_pos += len;
                    // Don't count mismatches as matches
                }
            }
        }

        let query_start = query_start.unwrap_or(0);
        let query_end = query_pos;
        let alignment_end = reference_pos;

        Ok((query_start, query_end, alignment_end, residue_matches))
    }

    /// Auto-detect file format and parse accordingly
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Vec<Anchor>> {
        let path_str = path.as_ref().to_string_lossy();
        
        if path_str.ends_with(".bam") {
            Self::parse_bam_file(path)
        } else {
            Self::parse_sam_file(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cigar_boundaries() {
        // Test a simple CIGAR string: 10M5I15M (10 matches, 5 insertions, 15 matches)
        use noodles::sam::alignment::record::cigar::{op::Kind, Op};
        use noodles::sam::alignment::record::Cigar;
        
        let ops = vec![
            Op::new(Kind::Match, 10),
            Op::new(Kind::Insertion, 5),
            Op::new(Kind::Match, 15),
        ];
        
        let cigar = Cigar::try_from(ops).unwrap();
        let (query_start, query_end, alignment_end, matches) = 
            SamParser::parse_cigar_boundaries(&cigar, 100).unwrap();
        
        assert_eq!(query_start, 0);      // Query starts at position 0
        assert_eq!(query_end, 30);       // 10 + 5 + 15 = 30 query bases consumed
        assert_eq!(alignment_end, 125);  // 100 + 10 + 15 = 125 (insertion doesn't consume reference)
        assert_eq!(matches, 25);         // 10 + 15 = 25 matches
    }

    #[test]
    fn test_parse_cigar_with_deletions() {
        // Test CIGAR: 10M5D10M (10 matches, 5 deletions, 10 matches)
        use noodles::sam::alignment::record::cigar::{op::Kind, Op};
        use noodles::sam::alignment::record::Cigar;
        
        let ops = vec![
            Op::new(Kind::Match, 10),
            Op::new(Kind::Deletion, 5),
            Op::new(Kind::Match, 10),
        ];
        
        let cigar = Cigar::try_from(ops).unwrap();
        let (query_start, query_end, alignment_end, matches) = 
            SamParser::parse_cigar_boundaries(&cigar, 100).unwrap();
        
        assert_eq!(query_start, 0);
        assert_eq!(query_end, 20);       // 10 + 10 = 20 query bases (deletion doesn't consume query)
        assert_eq!(alignment_end, 125);  // 100 + 10 + 5 + 10 = 125
        assert_eq!(matches, 20);         // 10 + 10 = 20 matches
    }

    #[test]
    fn test_parse_cigar_with_soft_clipping() {
        // Test CIGAR: 5S20M5S (5 soft clip, 20 matches, 5 soft clip)
        use noodles::sam::alignment::record::cigar::{op::Kind, Op};
        use noodles::sam::alignment::record::Cigar;
        
        let ops = vec![
            Op::new(Kind::SoftClip, 5),
            Op::new(Kind::Match, 20),
            Op::new(Kind::SoftClip, 5),
        ];
        
        let cigar = Cigar::try_from(ops).unwrap();
        let (query_start, query_end, alignment_end, matches) = 
            SamParser::parse_cigar_boundaries(&cigar, 100).unwrap();
        
        assert_eq!(query_start, 5);      // Query alignment starts after soft clip
        assert_eq!(query_end, 30);       // 5 + 20 + 5 = 30 total query bases
        assert_eq!(alignment_end, 120);  // 100 + 20 = 120 (soft clips don't consume reference)
        assert_eq!(matches, 20);         // 20 matches
    }
}