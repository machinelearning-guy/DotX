//! MUMmer output file parser
//!
//! Parses output from MUMmer alignment tools including:
//! - .delta files from nucmer/promer
//! - .coords files from show-coords
//! - .cluster files from mummerplot

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use anyhow::{anyhow, Result};
use thiserror::Error;
use flate2::read::GzDecoder;
use regex::Regex;

use crate::types::{Anchor, Position, Strand};

#[derive(Debug, Error)]
pub enum MummerError {
    #[error("Invalid delta file format: {0}")]
    InvalidDeltaFormat(String),
    #[error("Invalid coords file format: {0}")]
    InvalidCoordsFormat(String),
    #[error("Invalid position value: {0}")]
    InvalidPosition(String),
    #[error("Missing header information")]
    MissingHeader,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}

/// Represents a MUMmer delta alignment
#[derive(Debug, Clone)]
pub struct DeltaAlignment {
    pub reference_name: String,
    pub query_name: String,
    pub reference_length: Position,
    pub query_length: Position,
    pub reference_start: Position,
    pub reference_end: Position,
    pub query_start: Position,
    pub query_end: Position,
    pub errors: u32,
    pub similarity_errors: u32,
    pub stop_codons: u32,
    pub deltas: Vec<i32>,
}

/// Represents coordinates from show-coords output
#[derive(Debug, Clone)]
pub struct CoordsAlignment {
    pub reference_start: Position,
    pub reference_end: Position,
    pub query_start: Position,
    pub query_end: Position,
    pub reference_aligned_length: Position,
    pub query_aligned_length: Position,
    pub identity: f64,
    pub reference_name: String,
    pub query_name: String,
}

/// MUMmer parser for reading various MUMmer output formats
pub struct MummerParser;

impl MummerParser {
    /// Parse a MUMmer delta file and return a vector of Anchors
    pub fn parse_delta_file<P: AsRef<Path>>(path: P) -> Result<Vec<Anchor>> {
        let file = File::open(&path)?;
        let path_str = path.as_ref().to_string_lossy();
        
        if path_str.ends_with(".gz") {
            let decoder = GzDecoder::new(file);
            let reader = BufReader::new(decoder);
            Self::parse_delta_reader(reader)
        } else {
            let reader = BufReader::new(file);
            Self::parse_delta_reader(reader)
        }
    }

    /// Parse delta format from a reader
    fn parse_delta_reader<R: BufRead>(reader: R) -> Result<Vec<Anchor>> {
        let mut lines = reader.lines();
        
        // Parse header line
        let header = lines.next()
            .ok_or(MummerError::MissingHeader)??;
        let header_parts: Vec<&str> = header.split_whitespace().collect();
        if header_parts.len() < 2 {
            return Err(MummerError::InvalidDeltaFormat("Invalid header".to_string()).into());
        }

        let mut anchors = Vec::new();
        let mut current_reference: Option<String> = None;
        let mut current_query: Option<String> = None;
        let mut ref_length: Position = 0;
        let mut query_length: Position = 0;

        let mut line_iter = lines.enumerate();
        while let Some((line_num, line)) = line_iter.next() {
            let line = line?;
            let trimmed = line.trim();
            
            if trimmed.is_empty() {
                continue;
            }

            // Check if this is a sequence header line (starts with '>')
            if trimmed.starts_with('>') {
                let parts: Vec<&str> = trimmed[1..].split_whitespace().collect();
                if parts.len() >= 4 {
                    current_reference = Some(parts[0].to_string());
                    current_query = Some(parts[1].to_string());
                    ref_length = parts[2].parse::<Position>()
                        .map_err(|_| MummerError::InvalidPosition(parts[2].to_string()))?;
                    query_length = parts[3].parse::<Position>()
                        .map_err(|_| MummerError::InvalidPosition(parts[3].to_string()))?;
                }
                continue;
            }

            // Parse alignment coordinates line
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 7 {
                let ref_start = parts[0].parse::<Position>()
                    .map_err(|_| MummerError::InvalidPosition(parts[0].to_string()))?;
                let ref_end = parts[1].parse::<Position>()
                    .map_err(|_| MummerError::InvalidPosition(parts[1].to_string()))?;
                let query_start = parts[2].parse::<Position>()
                    .map_err(|_| MummerError::InvalidPosition(parts[2].to_string()))?;
                let query_end = parts[3].parse::<Position>()
                    .map_err(|_| MummerError::InvalidPosition(parts[3].to_string()))?;
                let errors = parts[4].parse::<u32>()
                    .map_err(|_| MummerError::InvalidPosition(parts[4].to_string()))?;
                let sim_errors = parts[5].parse::<u32>()
                    .map_err(|_| MummerError::InvalidPosition(parts[5].to_string()))?;
                let stop_codons = parts[6].parse::<u32>()
                    .map_err(|_| MummerError::InvalidPosition(parts[6].to_string()))?;

                // Read delta values until we hit 0 or next alignment
                let mut deltas = Vec::new();
                while let Some((_, delta_line)) = line_iter.next() {
                    let delta_line = delta_line?;
                    let delta_trimmed = delta_line.trim();
                    
                    if delta_trimmed.is_empty() {
                        continue;
                    }

                    let delta_val = delta_trimmed.parse::<i32>()
                        .map_err(|_| MummerError::InvalidPosition(delta_trimmed.to_string()))?;
                    
                    if delta_val == 0 {
                        break;
                    }
                    deltas.push(delta_val);
                }

                if let (Some(ref ref_name), Some(ref query_name)) = (&current_reference, &current_query) {
                    let anchor = Self::delta_to_anchor(
                        ref_name.clone(),
                        query_name.clone(),
                        ref_length,
                        query_length,
                        ref_start,
                        ref_end,
                        query_start,
                        query_end,
                        errors,
                        sim_errors,
                        stop_codons,
                        &deltas,
                    )?;
                    anchors.push(anchor);
                }
            }
        }

        Ok(anchors)
    }

    /// Convert delta alignment information to an Anchor
    fn delta_to_anchor(
        reference_name: String,
        query_name: String,
        reference_length: Position,
        query_length: Position,
        ref_start: Position,
        ref_end: Position,
        query_start: Position,
        query_end: Position,
        errors: u32,
        similarity_errors: u32,
        stop_codons: u32,
        deltas: &[i32],
    ) -> Result<Anchor> {
        // Determine strand based on coordinate order
        let strand = if query_start <= query_end {
            Strand::Forward
        } else {
            Strand::Reverse
        };

        // Normalize coordinates for reverse strand
        let (norm_query_start, norm_query_end) = if strand == Strand::Reverse {
            (query_end, query_start)
        } else {
            (query_start, query_end)
        };

        // Calculate alignment statistics
        let alignment_length: u64 = (ref_end - ref_start + 1).max(norm_query_end - norm_query_start + 1);
        let residue_matches = if alignment_length > errors as u64 {
            (alignment_length - errors as u64) as u32
        } else {
            0
        };

        let mut anchor = Anchor::from_parser(
            query_name,
            query_length,
            norm_query_start - 1, // Convert to 0-based coordinates
            norm_query_end,
            strand,
            reference_name,
            reference_length,
            ref_start - 1, // Convert to 0-based coordinates
            ref_end,
            residue_matches,
            alignment_length,
        );

        // Add MUMmer-specific tags
        anchor = anchor.with_tag("errors".to_string(), errors.to_string());
        anchor = anchor.with_tag("similarity_errors".to_string(), similarity_errors.to_string());
        anchor = anchor.with_tag("stop_codons".to_string(), stop_codons.to_string());
        anchor = anchor.with_tag("delta_count".to_string(), deltas.len().to_string());

        Ok(anchor)
    }

    /// Parse a MUMmer coords file (output from show-coords)
    pub fn parse_coords_file<P: AsRef<Path>>(path: P) -> Result<Vec<Anchor>> {
        let file = File::open(&path)?;
        let path_str = path.as_ref().to_string_lossy();
        
        if path_str.ends_with(".gz") {
            let decoder = GzDecoder::new(file);
            let reader = BufReader::new(decoder);
            Self::parse_coords_reader(reader)
        } else {
            let reader = BufReader::new(file);
            Self::parse_coords_reader(reader)
        }
    }

    /// Parse coords format from a reader
    fn parse_coords_reader<R: BufRead>(reader: R) -> Result<Vec<Anchor>> {
        let mut anchors = Vec::new();
        let mut header_skipped = false;

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            
            // Skip empty lines, comments, and header lines
            if trimmed.is_empty() || trimmed.starts_with('#') || 
               trimmed.starts_with("NUCMER") || trimmed.starts_with("=") {
                continue;
            }

            // Skip the column header line (contains "REF" and "QUERY")
            if !header_skipped {
                if trimmed.contains("REF") && trimmed.contains("QUERY") {
                    header_skipped = true;
                    continue;
                }
                // Skip any additional header/separator lines
                if trimmed.chars().all(|c| c.is_whitespace() || c == '=' || c == '-') {
                    continue;
                }
            }

            // Parse coordinates line
            if let Ok(anchor) = Self::parse_coords_line(trimmed) {
                anchors.push(anchor);
            }
        }

        Ok(anchors)
    }

    /// Parse a single line from coords output
    fn parse_coords_line(line: &str) -> Result<Anchor, MummerError> {
        // Typical coords line format (tab or space separated):
        // REF_START REF_END QUERY_START QUERY_END REF_LEN QUERY_LEN IDENTITY REF_NAME QUERY_NAME
        
        let parts: Vec<&str> = if line.contains('\t') {
            line.split('\t').collect()
        } else {
            line.split_whitespace().collect()
        };

        if parts.len() < 9 {
            return Err(MummerError::InvalidCoordsFormat(format!(
                "Expected at least 9 fields, got {}", parts.len()
            )));
        }

        let ref_start = parts[0].parse::<Position>()
            .map_err(|_| MummerError::InvalidPosition(parts[0].to_string()))?;
        let ref_end = parts[1].parse::<Position>()
            .map_err(|_| MummerError::InvalidPosition(parts[1].to_string()))?;
        let query_start = parts[2].parse::<Position>()
            .map_err(|_| MummerError::InvalidPosition(parts[2].to_string()))?;
        let query_end = parts[3].parse::<Position>()
            .map_err(|_| MummerError::InvalidPosition(parts[3].to_string()))?;
        let ref_aligned_len = parts[4].parse::<Position>()
            .map_err(|_| MummerError::InvalidPosition(parts[4].to_string()))?;
        let query_aligned_len = parts[5].parse::<Position>()
            .map_err(|_| MummerError::InvalidPosition(parts[5].to_string()))?;
        let identity = parts[6].parse::<f64>()
            .map_err(|_| MummerError::InvalidPosition(parts[6].to_string()))?;
        let ref_name = parts[7].to_string();
        let query_name = parts[8].to_string();

        // Determine strand based on coordinate order
        let strand = if query_start <= query_end {
            Strand::Forward
        } else {
            Strand::Reverse
        };

        // Normalize coordinates for reverse strand
        let (norm_query_start, norm_query_end) = if strand == Strand::Reverse {
            (query_end, query_start)
        } else {
            (query_start, query_end)
        };

        // Calculate residue matches from identity percentage
        let alignment_length = ref_aligned_len.max(query_aligned_len);
        let residue_matches = ((identity / 100.0) * alignment_length as f64) as u32;

        // We don't have sequence lengths from coords files, so use aligned lengths as approximations
        let mut anchor = Anchor::from_parser(
            query_name,
            norm_query_end.max(query_aligned_len), // Approximation
            norm_query_start - 1, // Convert to 0-based
            norm_query_end,
            strand,
            ref_name,
            ref_end.max(ref_aligned_len), // Approximation
            ref_start - 1, // Convert to 0-based
            ref_end,
            residue_matches,
            alignment_length,
        );

        // Add coords-specific tags
        anchor = anchor.with_tag("identity".to_string(), identity.to_string());
        anchor = anchor.with_tag("ref_aligned_length".to_string(), ref_aligned_len.to_string());
        anchor = anchor.with_tag("query_aligned_length".to_string(), query_aligned_len.to_string());

        Ok(anchor)
    }

    /// Auto-detect MUMmer file format and parse accordingly
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Vec<Anchor>> {
        let path_str = path.as_ref().to_string_lossy();
        
        if path_str.contains(".delta") {
            Self::parse_delta_file(path)
        } else if path_str.contains(".coords") {
            Self::parse_coords_file(path)
        } else {
            // Try to auto-detect by reading first few lines
            let file = File::open(&path)?;
            let mut reader = BufReader::new(file);
            let mut first_line = String::new();
            reader.read_line(&mut first_line)?;
            
            if first_line.trim().starts_with('>') || first_line.split_whitespace().count() == 2 {
                // Likely a delta file
                drop(reader);
                Self::parse_delta_file(path)
            } else {
                // Assume coords file
                drop(reader);
                Self::parse_coords_file(path)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_coords_line() {
        let line = "1000\t2000\t500\t1500\t1001\t1001\t85.50\tref_seq\tquery_seq";
        let anchor = MummerParser::parse_coords_line(line).unwrap();
        
        assert_eq!(anchor.t, "ref_seq");
        assert_eq!(anchor.q, "query_seq");
        assert_eq!(anchor.ts, 999); // 0-based
        assert_eq!(anchor.te, 2000);
        assert_eq!(anchor.qs, 499); // 0-based
        assert_eq!(anchor.qe, 1500);
        assert_eq!(anchor.strand, Strand::Forward);
        assert_eq!(anchor.tags.get("identity"), Some(&"85.5".to_string()));
    }

    #[test]
    fn test_parse_coords_line_reverse() {
        let line = "1000\t2000\t1500\t500\t1001\t1001\t90.25\tref_seq\tquery_seq";
        let anchor = MummerParser::parse_coords_line(line).unwrap();
        
        assert_eq!(anchor.strand, Strand::Reverse);
        assert_eq!(anchor.qs, 499); // Normalized and 0-based
        assert_eq!(anchor.qe, 1500);
    }

    #[test]
    fn test_parse_coords_reader() {
        let coords_data = "# NUCMER output\n\
                          REF_START\tREF_END\tQUERY_START\tQUERY_END\tREF_LEN\tQUERY_LEN\tIDENTITY\tREF_NAME\tQUERY_NAME\n\
                          1000\t2000\t500\t1500\t1001\t1001\t85.50\tref_seq\tquery_seq\n\
                          3000\t4000\t2500\t3500\t1001\t1001\t92.75\tref_seq\tquery_seq2\n";
        
        let cursor = Cursor::new(coords_data);
        let anchors = MummerParser::parse_coords_reader(cursor).unwrap();
        
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].q, "query_seq");
        assert_eq!(anchors[1].q, "query_seq2");
        assert_eq!(anchors[1].tags.get("identity"), Some(&"92.75".to_string()));
    }

    #[test]
    fn test_delta_to_anchor() {
        let anchor = MummerParser::delta_to_anchor(
            "ref_seq".to_string(),
            "query_seq".to_string(),
            10000,
            8000,
            1000,
            2000,
            500,
            1500,
            50,
            10,
            0,
            &vec![100, -50, 25],
        ).unwrap();
        
        assert_eq!(anchor.t, "ref_seq");
        assert_eq!(anchor.q, "query_seq");
        assert_eq!(anchor.strand, Strand::Forward);
        assert_eq!(anchor.tags.get("errors"), Some(&"50".to_string()));
        assert_eq!(anchor.tags.get("delta_count"), Some(&"3".to_string()));
    }

    #[test]
    fn test_delta_to_anchor_reverse() {
        let anchor = MummerParser::delta_to_anchor(
            "ref_seq".to_string(),
            "query_seq".to_string(),
            10000,
            8000,
            1000,
            2000,
            1500,  // query_start > query_end
            500,
            30,
            5,
            0,
            &vec![],
        ).unwrap();
        
        assert_eq!(anchor.strand, Strand::Reverse);
        assert_eq!(anchor.qs, 499); // Normalized and 0-based
        assert_eq!(anchor.qe, 1500);
    }

    #[test]
    fn test_parse_insufficient_coords_fields() {
        let line = "1000\t2000\t500\t1500"; // Only 4 fields instead of 9
        let result = MummerParser::parse_coords_line(line);
        assert!(matches!(result, Err(MummerError::InvalidCoordsFormat(_))));
    }
}
