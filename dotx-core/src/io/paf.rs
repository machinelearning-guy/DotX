//! PAF (Pairwise mApping Format) file parser
//! 
//! PAF is a text format used to describe the approximate mapping positions
//! between two sets of sequences. It consists of at least 12 fields separated by tabs.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use flate2::read::GzDecoder;
use anyhow::{anyhow, Result};
use thiserror::Error;

use crate::types::{Anchor, Position, Strand};

#[derive(Debug, Error)]
pub enum PafError {
    #[error("Invalid PAF line: insufficient fields (expected at least 12, got {0})")]
    InsufficientFields(usize),
    #[error("Invalid position value: {0}")]
    InvalidPosition(String),
    #[error("Invalid strand: {0}")]
    InvalidStrand(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
}

/// PAF parser for reading alignment records
pub struct PafParser;

impl PafParser {
    /// Parse a single PAF line into an Anchor
    pub fn parse_line(line: &str) -> Result<Anchor, PafError> {
        let fields: Vec<&str> = line.split('\t').collect();
        
        if fields.len() < 12 {
            return Err(PafError::InsufficientFields(fields.len()));
        }

        // Parse the 12 mandatory PAF fields
        let query_name = fields[0].to_string();
        let query_length = fields[1].parse::<Position>()
            .map_err(|_| PafError::InvalidPosition(fields[1].to_string()))?;
        let query_start = fields[2].parse::<Position>()
            .map_err(|_| PafError::InvalidPosition(fields[2].to_string()))?;
        let query_end = fields[3].parse::<Position>()
            .map_err(|_| PafError::InvalidPosition(fields[3].to_string()))?;
        
        let strand = match fields[4] {
            "+" => Strand::Forward,
            "-" => Strand::Reverse,
            s => return Err(PafError::InvalidStrand(s.to_string())),
        };
        
        let target_name = fields[5].to_string();
        let target_length = fields[6].parse::<Position>()
            .map_err(|_| PafError::InvalidPosition(fields[6].to_string()))?;
        let target_start = fields[7].parse::<Position>()
            .map_err(|_| PafError::InvalidPosition(fields[7].to_string()))?;
        let target_end = fields[8].parse::<Position>()
            .map_err(|_| PafError::InvalidPosition(fields[8].to_string()))?;
        
        let residue_matches = fields[9].parse::<u32>()
            .map_err(|_| PafError::Parse(format!("Invalid residue matches: {}", fields[9])))?;
        let alignment_block_length = fields[10].parse::<Position>()
            .map_err(|_| PafError::InvalidPosition(fields[10].to_string()))?;
        
        let mapping_quality = if fields[11] == "255" || fields[11] == "*" {
            None
        } else {
            Some(fields[11].parse::<u8>()
                .map_err(|_| PafError::Parse(format!("Invalid mapping quality: {}", fields[11])))?)
        };

        let mut anchor = Anchor::from_parser(
            query_name,
            query_length,
            query_start,
            query_end,
            strand,
            target_name,
            target_length,
            target_start,
            target_end,
            residue_matches,
            alignment_block_length,
        );

        if let Some(quality) = mapping_quality {
            anchor = anchor.with_mapping_quality(quality);
        }

        // Parse optional tags (fields 12+)
        for field in fields.iter().skip(12) {
            if let Some((tag, value)) = Self::parse_tag(field) {
                anchor = anchor.with_tag(tag, value);
            }
        }

        Ok(anchor)
    }

    /// Parse PAF optional tags in the format "XX:Y:value"
    /// where XX is the tag name, Y is the type, and value is the value
    fn parse_tag(tag_field: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = tag_field.split(':').collect();
        if parts.len() >= 3 {
            let tag_name = parts[0].to_string();
            let tag_value = parts[2..].join(":"); // Join in case value contains colons
            Some((tag_name, tag_value))
        } else {
            None
        }
    }

    /// Parse a PAF file and return a vector of Anchors
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

    /// Parse PAF data from any BufRead source
    pub fn parse_reader<R: BufRead>(reader: R) -> Result<Vec<Anchor>> {
        let mut anchors = Vec::new();
        
        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            match Self::parse_line(&line) {
                Ok(anchor) => anchors.push(anchor),
                Err(e) => return Err(anyhow!("Error parsing line {}: {}", line_num + 1, e)),
            }
        }
        
        Ok(anchors)
    }

    /// Create an iterator over anchors from a PAF file
    pub fn iter_file<P: AsRef<Path>>(path: P) -> Result<PafIterator<BufReader<Box<dyn std::io::Read>>>> {
        let file = File::open(&path)?;
        let path_str = path.as_ref().to_string_lossy();
        
        let reader: Box<dyn std::io::Read> = if path_str.ends_with(".gz") {
            Box::new(GzDecoder::new(file))
        } else {
            Box::new(file)
        };
        
        Ok(PafIterator::new(BufReader::new(reader)))
    }
}

/// Iterator over PAF entries
pub struct PafIterator<R: BufRead> {
    reader: R,
    line_buffer: String,
    line_number: usize,
}

impl<R: BufRead> PafIterator<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            line_buffer: String::new(),
            line_number: 0,
        }
    }
}

impl<R: BufRead> Iterator for PafIterator<R> {
    type Item = Result<Anchor>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.line_buffer.clear();
            
            match self.reader.read_line(&mut self.line_buffer) {
                Ok(0) => return None, // EOF
                Ok(_) => {
                    self.line_number += 1;
                    let line = self.line_buffer.trim();
                    
                    // Skip empty lines and comments
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    
                    match PafParser::parse_line(line) {
                        Ok(anchor) => return Some(Ok(anchor)),
                        Err(e) => return Some(Err(anyhow!(
                            "Error parsing line {}: {}", self.line_number, e
                        ))),
                    }
                }
                Err(e) => return Some(Err(e.into())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_basic_paf_line() {
        let line = "query1\t1000\t100\t900\t+\ttarget1\t2000\t500\t1300\t750\t800\t60\tAS:i:750";
        
        let anchor = PafParser::parse_line(line).unwrap();
        
        assert_eq!(anchor.q, "query1");
        assert_eq!(anchor.query_length, Some(1000));
        assert_eq!(anchor.qs, 100);
        assert_eq!(anchor.qe, 900);
        assert_eq!(anchor.strand, Strand::Forward);
        assert_eq!(anchor.t, "target1");
        assert_eq!(anchor.target_length, Some(2000));
        assert_eq!(anchor.ts, 500);
        assert_eq!(anchor.te, 1300);
        assert_eq!(anchor.residue_matches, Some(750));
        assert_eq!(anchor.alignment_block_length, Some(800));
        assert_eq!(anchor.mapq, Some(60));
        assert_eq!(anchor.tags.get("AS"), Some(&"750".to_string()));
    }

    #[test]
    fn test_parse_reverse_strand() {
        let line = "query1\t1000\t100\t900\t-\ttarget1\t2000\t500\t1300\t750\t800\t60";
        
        let anchor = PafParser::parse_line(line).unwrap();
        assert_eq!(anchor.strand, Strand::Reverse);
    }

    #[test]
    fn test_parse_no_mapping_quality() {
        let line = "query1\t1000\t100\t900\t+\ttarget1\t2000\t500\t1300\t750\t800\t255";
        
        let anchor = PafParser::parse_line(line).unwrap();
        assert_eq!(anchor.mapq, None);
    }

    #[test]
    fn test_parse_multiple_tags() {
        let line = "query1\t1000\t100\t900\t+\ttarget1\t2000\t500\t1300\t750\t800\t60\tAS:i:750\tNM:i:50\ttp:A:P";
        
        let anchor = PafParser::parse_line(line).unwrap();
        
        assert_eq!(anchor.tags.get("AS"), Some(&"750".to_string()));
        assert_eq!(anchor.tags.get("NM"), Some(&"50".to_string()));
        assert_eq!(anchor.tags.get("tp"), Some(&"P".to_string()));
    }

    #[test]
    fn test_parse_insufficient_fields() {
        let line = "query1\t1000\t100\t900\t+\ttarget1\t2000\t500";
        
        let result = PafParser::parse_line(line);
        assert!(matches!(result, Err(PafError::InsufficientFields(8))));
    }

    #[test]
    fn test_parse_invalid_strand() {
        let line = "query1\t1000\t100\t900\tx\ttarget1\t2000\t500\t1300\t750\t800\t60";
        
        let result = PafParser::parse_line(line);
        assert!(matches!(result, Err(PafError::InvalidStrand(_))));
    }

    #[test]
    fn test_parse_reader() {
        let paf_data = "query1\t1000\t100\t900\t+\ttarget1\t2000\t500\t1300\t750\t800\t60\n\
                        query2\t800\t50\t750\t-\ttarget2\t1500\t200\t900\t650\t700\t55\n";
        
        let cursor = Cursor::new(paf_data);
        let anchors = PafParser::parse_reader(cursor).unwrap();
        
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].q, "query1");
        assert_eq!(anchors[1].q, "query2");
        assert_eq!(anchors[1].strand, Strand::Reverse);
    }

    #[test]
    fn test_skip_comments_and_empty_lines() {
        let paf_data = "# This is a comment\n\
                        \n\
                        query1\t1000\t100\t900\t+\ttarget1\t2000\t500\t1300\t750\t800\t60\n\
                        \n\
                        # Another comment\n\
                        query2\t800\t50\t750\t-\ttarget2\t1500\t200\t900\t650\t700\t55\n";
        
        let cursor = Cursor::new(paf_data);
        let anchors = PafParser::parse_reader(cursor).unwrap();
        
        assert_eq!(anchors.len(), 2);
    }

    #[test]
    fn test_iterator() {
        let paf_data = "query1\t1000\t100\t900\t+\ttarget1\t2000\t500\t1300\t750\t800\t60\n\
                        query2\t800\t50\t750\t-\ttarget2\t1500\t200\t900\t650\t700\t55\n";
        
        let cursor = Cursor::new(paf_data);
        let mut iterator = PafIterator::new(cursor);
        
        let first = iterator.next().unwrap().unwrap();
        assert_eq!(first.q, "query1");
        
        let second = iterator.next().unwrap().unwrap();
        assert_eq!(second.q, "query2");
        
        assert!(iterator.next().is_none());
    }
}
