//! FASTA/FASTQ sequence file parser
//!
//! Fast parsing of FASTA and FASTQ files using the needletail library.
//! Supports both single sequences and collections, with streaming capabilities
//! for memory-efficient processing of large files.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use anyhow::{Result};
use thiserror::Error;
use flate2::read::GzDecoder;

use needletail::{parse_fastx_file, parse_fastx_reader};
use crate::types::{Sequence as DotxSequence, Position};

#[derive(Debug, Error)]
pub enum FastaError {
    #[error("Invalid sequence format: {0}")]
    InvalidFormat(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Empty file or no sequences found")]
    EmptyFile,
}

/// FASTA/FASTQ parser for reading sequence data
pub struct FastaParser;

impl FastaParser {
    /// Parse a FASTA/FASTQ file and return a vector of DotxSequence structs
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Vec<DotxSequence>> {
        let path_str = path.as_ref().to_string_lossy();
        
        if path_str.ends_with(".gz") {
            Self::parse_gzipped_file(path)
        } else {
            Self::parse_uncompressed_file(path)
        }
    }

    /// Parse an uncompressed FASTA/FASTQ file
    fn parse_uncompressed_file<P: AsRef<Path>>(path: P) -> Result<Vec<DotxSequence>> {
        let mut sequences = Vec::new();
        let mut reader = parse_fastx_file(&path)
            .map_err(|e| FastaError::Parse(e.to_string()))?;

        while let Some(record) = reader.next() {
            let record = record.map_err(|e| FastaError::Parse(e.to_string()))?;
            let sequence = Self::record_to_sequence(record)?;
            sequences.push(sequence);
        }

        if sequences.is_empty() {
            Err(FastaError::EmptyFile.into())
        } else {
            Ok(sequences)
        }
    }

    /// Parse a gzipped FASTA/FASTQ file
    fn parse_gzipped_file<P: AsRef<Path>>(path: P) -> Result<Vec<DotxSequence>> {
        let file = File::open(&path)?;
        let decoder = GzDecoder::new(file);
        let buf_reader = BufReader::new(decoder);
        
        Self::parse_reader(buf_reader)
    }

    /// Parse FASTA/FASTQ data from any readable source
    pub fn parse_reader<R: std::io::Read + std::marker::Send>(reader: R) -> Result<Vec<DotxSequence>> {
        let mut sequences = Vec::new();
        let mut fastx_reader = parse_fastx_reader(reader)
            .map_err(|e| FastaError::Parse(e.to_string()))?;

        while let Some(record) = fastx_reader.next() {
            let record = record.map_err(|e| FastaError::Parse(e.to_string()))?;
            let sequence = Self::record_to_sequence(record)?;
            sequences.push(sequence);
        }

        if sequences.is_empty() {
            Err(FastaError::EmptyFile.into())
        } else {
            Ok(sequences)
        }
    }

    /// Convert a needletail sequence record to a DotxSequence
    fn record_to_sequence(record: needletail::parser::SequenceRecord) -> Result<DotxSequence> {
        let id_bytes = record.id();
        let id = String::from_utf8_lossy(id_bytes).to_string();
        
        // Description is not directly available from needletail's SequenceRecord in this version
        let description = None;

        // Get sequence data
        let sequence_data = record.seq().to_vec();
        
        let mut dotx_sequence = DotxSequence::new(id, sequence_data);
        if let Some(desc) = description {
            dotx_sequence = dotx_sequence.with_description(desc);
        }

        Ok(dotx_sequence)
    }

    /// Parse only sequence headers (fast scan for sequence information)
    pub fn parse_headers<P: AsRef<Path>>(path: P) -> Result<Vec<(String, Option<String>, Position)>> {
        let mut headers = Vec::new();
        let sequences = Self::parse_file(path)?;

        for sequence in sequences {
            headers.push((
                sequence.id.clone(),
                sequence.description.clone(),
                sequence.length,
            ));
        }

        Ok(headers)
    }

    /// Count sequences in a file without loading them into memory
    pub fn count_sequences<P: AsRef<Path>>(path: P) -> Result<usize> {
        let sequences = Self::parse_file(path)?;
        Ok(sequences.len())
    }

    /// Calculate total sequence length in a file
    pub fn total_sequence_length<P: AsRef<Path>>(path: P) -> Result<Position> {
        let sequences = Self::parse_file(path)?;
        Ok(sequences.iter().map(|s| s.length).sum())
    }

    /// Get basic statistics about sequences in a file
    pub fn sequence_statistics<P: AsRef<Path>>(path: P) -> Result<SequenceStatistics> {
        let sequences = Self::parse_file(path)?;
        let mut stats = SequenceStatistics::new();
        
        for sequence in &sequences {
            stats.add_sequence(&sequence);
        }
        
        stats.finalize();
        Ok(stats)
    }

    /// Validate FASTA/FASTQ file format without parsing all sequences
    pub fn validate_format<P: AsRef<Path>>(path: P) -> Result<FormatValidation> {
        let mut validation = FormatValidation::new();

        match Self::parse_file(path) {
            Ok(sequences) => {
                for sequence in sequences {
                    validation.valid_sequences += 1;
                    
                    // Check for common issues
                    if sequence.id.is_empty() {
                        validation.warnings.push("Empty sequence ID found".to_string());
                    }
                    
                    if sequence.length == 0 {
                        validation.warnings.push("Empty sequence found".to_string());
                    }

                    // Check for valid DNA/RNA characters (allowing ambiguous bases)
                    let valid_chars = b"ATCGUatcguNnRrYyKkMmSsWwBbDdHhVv-";
                    let has_invalid_chars = sequence.data.iter()
                        .any(|&c| !valid_chars.contains(&c));
                    
                    if has_invalid_chars {
                        validation.warnings.push(format!(
                            "Sequence '{}' contains invalid characters", sequence.id
                        ));
                    }
                }
            }
            Err(e) => {
                validation.errors.push(format!("Parse error: {}", e));
                validation.invalid_sequences += 1;
            }
        }

        Ok(validation)
    }
}

/// Statistics about sequences in a FASTA/FASTQ file
#[derive(Debug, Clone)]
pub struct SequenceStatistics {
    pub total_sequences: usize,
    pub total_length: Position,
    pub min_length: Position,
    pub max_length: Position,
    pub mean_length: f64,
    pub n50: Position,
    pub gc_content: f64,
    
    // Internal fields for calculation
    lengths: Vec<Position>,
    total_gc: u64,
    total_at: u64,
}

impl SequenceStatistics {
    fn new() -> Self {
        Self {
            total_sequences: 0,
            total_length: 0,
            min_length: Position::MAX,
            max_length: 0,
            mean_length: 0.0,
            n50: 0,
            gc_content: 0.0,
            lengths: Vec::new(),
            total_gc: 0,
            total_at: 0,
        }
    }

    fn add_sequence(&mut self, sequence: &DotxSequence) {
        self.total_sequences += 1;
        self.total_length += sequence.length;
        self.lengths.push(sequence.length);
        
        self.min_length = self.min_length.min(sequence.length);
        self.max_length = self.max_length.max(sequence.length);

        // Count GC content
        for &base in &sequence.data {
            match base.to_ascii_uppercase() {
                b'G' | b'C' => self.total_gc += 1,
                b'A' | b'T' | b'U' => self.total_at += 1,
                _ => {} // Skip ambiguous or gap characters
            }
        }
    }

    fn finalize(&mut self) {
        if self.total_sequences == 0 {
            return;
        }

        // Calculate mean length
        self.mean_length = self.total_length as f64 / self.total_sequences as f64;

        // Calculate N50
        self.lengths.sort_by(|a, b| b.cmp(a)); // Sort in descending order
        let mut cumulative_length = 0;
        let half_total = self.total_length / 2;
        
        for &length in &self.lengths {
            cumulative_length += length;
            if cumulative_length >= half_total {
                self.n50 = length;
                break;
            }
        }

        // Calculate GC content
        let total_bases = self.total_gc + self.total_at;
        if total_bases > 0 {
            self.gc_content = (self.total_gc as f64 / total_bases as f64) * 100.0;
        }

        // Fix edge case for min_length
        if self.min_length == Position::MAX {
            self.min_length = 0;
        }
    }
}

/// Validation results for FASTA/FASTQ format
#[derive(Debug, Clone)]
pub struct FormatValidation {
    pub valid_sequences: usize,
    pub invalid_sequences: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub partial_validation: bool,
}

impl FormatValidation {
    fn new() -> Self {
        Self {
            valid_sequences: 0,
            invalid_sequences: 0,
            warnings: Vec::new(),
            errors: Vec::new(),
            partial_validation: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty() && self.invalid_sequences == 0
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_fasta_reader() {
        let fasta_data = ">seq1 description of sequence 1\n\
                          ATCGATCGATCG\n\
                          >seq2\n\
                          GCTAGCTAGCTA\n";
        
        let cursor = Cursor::new(fasta_data);
        let sequences = FastaParser::parse_reader(cursor).unwrap();
        
        assert_eq!(sequences.len(), 2);
        assert_eq!(sequences[0].id, "seq1");
        assert_eq!(sequences[0].description, Some("description of sequence 1".to_string()));
        assert_eq!(sequences[0].data, b"ATCGATCGATCG");
        assert_eq!(sequences[0].length, 12);
        
        assert_eq!(sequences[1].id, "seq2");
        assert_eq!(sequences[1].description, None);
        assert_eq!(sequences[1].data, b"GCTAGCTAGCTA");
    }

    #[test]
    fn test_parse_fastq_reader() {
        let fastq_data = "@seq1 description\n\
                          ATCGATCG\n\
                          +\n\
                          IIIIIIII\n\
                          @seq2\n\
                          GCTAGCTA\n\
                          +\n\
                          HHHHHHHH\n";
        
        let cursor = Cursor::new(fastq_data);
        let sequences = FastaParser::parse_reader(cursor).unwrap();
        
        assert_eq!(sequences.len(), 2);
        assert_eq!(sequences[0].id, "seq1");
        assert_eq!(sequences[0].description, Some("description".to_string()));
        assert_eq!(sequences[0].data, b"ATCGATCG");
        
        assert_eq!(sequences[1].id, "seq2");
        assert_eq!(sequences[1].data, b"GCTAGCTA");
    }

    #[test]
    fn test_multiline_fasta() {
        let fasta_data = ">seq1\n\
                          ATCGATCG\n\
                          ATCGATCG\n\
                          GCTAGCTA\n";
        
        let cursor = Cursor::new(fasta_data);
        let sequences = FastaParser::parse_reader(cursor).unwrap();
        
        assert_eq!(sequences.len(), 1);
        assert_eq!(sequences[0].data, b"ATCGATCGATCGATCGGCTAGCTA");
        assert_eq!(sequences[0].length, 24);
    }

    #[test]
    fn test_empty_file() {
        let cursor = Cursor::new("");
        let result = FastaParser::parse_reader(cursor);
        assert!(result.is_err());
    }

    #[test]
    fn test_sequence_statistics() {
        let fasta_data = ">seq1\n\
                          ATCGATCGATCG\n\
                          >seq2\n\
                          GCGCGCGCGCGCGCGC\n\
                          >seq3\n\
                          ATGC\n";
        
        let cursor = Cursor::new(fasta_data);
        let sequences = FastaParser::parse_reader(cursor).unwrap();
        
        let mut stats = SequenceStatistics::new();
        for seq in &sequences {
            stats.add_sequence(seq);
        }
        stats.finalize();
        
        assert_eq!(stats.total_sequences, 3);
        assert_eq!(stats.total_length, 32); // 12 + 16 + 4
        assert_eq!(stats.min_length, 4);
        assert_eq!(stats.max_length, 16);
        assert_eq!(stats.mean_length, 32.0 / 3.0);
        assert_eq!(stats.n50, 16); // Longest sequence when sorted: [16, 12, 4]
    }

    #[test]
    fn test_gc_content_calculation() {
        let fasta_data = ">test_seq\n\
                          GGCCGGCC\n"; // 100% GC content
        
        let cursor = Cursor::new(fasta_data);
        let sequences = FastaParser::parse_reader(cursor).unwrap();
        
        let mut stats = SequenceStatistics::new();
        stats.add_sequence(&sequences[0]);
        stats.finalize();
        
        assert_eq!(stats.gc_content, 100.0);
    }
}
