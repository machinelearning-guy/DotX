//! File format I/O modules for DOTx
//!
//! This module provides parsers for various bioinformatics file formats,
//! converting them to unified Anchor representations for alignment visualization.

#[cfg(feature = "io-paf")] pub mod paf;
#[cfg(feature = "io-sam")] pub mod sam;
#[cfg(feature = "io-maf")] pub mod maf;
#[cfg(feature = "io-mummer")] pub mod mummer;
#[cfg(feature = "io-fasta")] pub mod fasta;

#[cfg(feature = "io-paf")] pub use paf::{PafParser, PafError};
#[cfg(feature = "io-sam")] pub use sam::{SamParser, SamError};
#[cfg(feature = "io-maf")] pub use maf::{MafParser, MafError, MafBlock, MafSequence};
#[cfg(feature = "io-mummer")] pub use mummer::{MummerParser, MummerError, DeltaAlignment, CoordsAlignment};
#[cfg(feature = "io-fasta")] pub use fasta::{FastaParser, FastaError, SequenceStatistics, FormatValidation};

use anyhow::Result;
use std::path::Path;
use crate::types::Anchor;

/// Auto-detect file format and parse accordingly
pub fn parse_alignment_file<P: AsRef<Path>>(path: P) -> Result<Vec<Anchor>> {
    let path_str = path.as_ref().to_string_lossy().to_lowercase();
    
    #[cfg(feature = "io-paf")]
    if path_str.ends_with(".paf") || path_str.ends_with(".paf.gz") { return PafParser::parse_file(path); }
    #[cfg(feature = "io-sam")]
    if path_str.ends_with(".sam") || path_str.ends_with(".bam") { return SamParser::parse_file(path); }
    #[cfg(feature = "io-maf")]
    if path_str.ends_with(".maf") || path_str.ends_with(".maf.gz") { return MafParser::parse_file(path); }
    #[cfg(feature = "io-mummer")]
    if path_str.contains(".delta") || path_str.contains(".coords") { return MummerParser::parse_file(path); }
    // Fallback to auto-detect
    auto_detect_and_parse(path)
}

/// Auto-detect file format by examining file content
fn auto_detect_and_parse<P: AsRef<Path>>(path: P) -> Result<Vec<Anchor>> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use flate2::read::GzDecoder;
    
    let file = File::open(&path)?;
    let path_str = path.as_ref().to_string_lossy();
    
    let first_line = if path_str.ends_with(".gz") {
        let decoder = GzDecoder::new(file);
        let mut reader = BufReader::new(decoder);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        line
    } else {
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        line
    };
    
    let trimmed = first_line.trim();
    
    // Detect format based on first line characteristics
    #[cfg(feature = "io-sam")]
    if trimmed.starts_with('@') && (trimmed.contains("HD:VN:") || trimmed.contains("SQ:SN:")) { return SamParser::parse_sam_file(path); }
    #[cfg(feature = "io-paf")]
    if trimmed.starts_with('#') && trimmed.to_lowercase().contains("paf") { return PafParser::parse_file(path); }
    #[cfg(feature = "io-maf")]
    if trimmed.starts_with("##maf") || trimmed.starts_with("a score=") { return MafParser::parse_file(path); }
    #[cfg(feature = "io-mummer")]
    if trimmed.starts_with('>') && trimmed.split_whitespace().count() == 4 { return MummerParser::parse_delta_file(path); }
    #[cfg(feature = "io-mummer")]
    if trimmed.contains("NUCMER") || (trimmed.split_whitespace().count() >= 9 && trimmed.split_whitespace().nth(6).and_then(|s| s.parse::<f64>().ok()).is_some()) { return MummerParser::parse_coords_file(path); }
    // Default to PAF if enabled, otherwise empty
    #[cfg(feature = "io-paf")]
    { return PafParser::parse_file(path); }
    #[allow(unreachable_code)]
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_auto_detect_paf() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "query1\t1000\t100\t900\t+\ttarget1\t2000\t500\t1300\t750\t800\t60").unwrap();
        
        let anchors = parse_alignment_file(file.path()).unwrap();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].q, "query1");
    }

    #[cfg(feature = "io-maf")]
    #[test]
    fn test_auto_detect_maf() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "##maf version=1").unwrap();
        writeln!(file, "a score=12345").unwrap();
        writeln!(file, "s hg18.chr7    27707221 13 + 158545518 gcagctgaaaaca").unwrap();
        writeln!(file, "s panTro1.chr6 28869787 13 - 161576975 gcagctgaaaaca").unwrap();
        
        let anchors = parse_alignment_file(file.path()).unwrap();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].t, "hg18.chr7");
    }

    #[cfg(feature = "io-mummer")]
    #[test]
    fn test_auto_detect_coords() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "1000\t2000\t500\t1500\t1001\t1001\t85.50\tref_seq\tquery_seq").unwrap();
        
        let anchors = parse_alignment_file(file.path()).unwrap();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].t, "ref_seq");
    }
}
