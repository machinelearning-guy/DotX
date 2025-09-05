//! FASTA file parsing and sequence handling

use std::path::Path;
use std::io::{BufRead, BufReader};
use std::fs::File;

/// Represents a FASTA sequence record
#[derive(Debug, Clone)]
pub struct FastaRecord {
    /// Sequence identifier (header line without >)
    pub id: String,
    /// Sequence data as bytes
    pub sequence: Vec<u8>,
}

/// Parse a FASTA file and return all sequences
pub fn parse_fasta<P: AsRef<Path>>(path: P) -> Result<Vec<FastaRecord>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    
    let mut records = Vec::new();
    let mut current_id: Option<String> = None;
    let mut current_seq = Vec::new();
    
    for line in reader.lines() {
        let line = line?;
        if line.starts_with('>') {
            // Save previous record if exists
            if let Some(id) = current_id.take() {
                records.push(FastaRecord {
                    id,
                    sequence: current_seq.clone(),
                });
                current_seq.clear();
            }
            // Start new record
            current_id = Some(line[1..].to_string());
        } else {
            // Add to current sequence
            current_seq.extend(line.trim().bytes());
        }
    }
    
    // Save final record
    if let Some(id) = current_id {
        records.push(FastaRecord {
            id,
            sequence: current_seq,
        });
    }
    
    Ok(records)
}

/// Load the first sequence from a FASTA file
pub fn load_single_sequence<P: AsRef<Path>>(path: P) -> Result<FastaRecord, Box<dyn std::error::Error>> {
    let mut records = parse_fasta(path)?;
    records.into_iter().next()
        .ok_or_else(|| "No sequences found in FASTA file".into())
}