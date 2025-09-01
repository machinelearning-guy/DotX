use crate::types::*;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PafRecord {
    pub query_name: String,
    pub query_len: GenomicPos,
    pub query_start: GenomicPos,
    pub query_end: GenomicPos,
    pub strand: Strand,
    pub target_name: String,
    pub target_len: GenomicPos,
    pub target_start: GenomicPos,
    pub target_end: GenomicPos,
    pub residue_matches: u64,
    pub alignment_len: u64,
    pub mapping_quality: u8,
    pub tags: Vec<(String, String, String)>, // (tag, type, value)
}

impl PafRecord {
    pub fn identity(&self) -> f64 {
        if self.alignment_len == 0 {
            0.0
        } else {
            self.residue_matches as f64 / self.alignment_len as f64
        }
    }

    pub fn query_aligned_len(&self) -> GenomicPos {
        self.query_end - self.query_start
    }

    pub fn target_aligned_len(&self) -> GenomicPos {
        self.target_end - self.target_start
    }

    pub fn to_line(&self) -> String {
        let mut line = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.query_name,
            self.query_len,
            self.query_start,
            self.query_end,
            char::from(self.strand),
            self.target_name,
            self.target_len,
            self.target_start,
            self.target_end,
            self.residue_matches,
            self.alignment_len,
            self.mapping_quality
        );

        for (tag, tag_type, value) in &self.tags {
            line.push_str(&format!("\t{}:{}:{}", tag, tag_type, value));
        }

        line
    }
}

impl FromStr for PafRecord {
    type Err = anyhow::Error;

    fn from_str(line: &str) -> Result<Self> {
        let line = line.trim();
        if line.is_empty() {
            return Err(anyhow!("Empty PAF line"));
        }
        
        let fields: Vec<&str> = line.split('\t').collect();
        
        if fields.len() < 12 {
            return Err(anyhow!("PAF line has only {} fields, expected at least 12", fields.len()));
        }

        // Validate and parse query information
        let query_name = fields[0].trim().to_string();
        if query_name.is_empty() {
            return Err(anyhow!("Query name cannot be empty"));
        }
        
        let query_len: GenomicPos = fields[1].parse().map_err(|_| anyhow!("Invalid query length: {}", fields[1]))?;
        let query_start: GenomicPos = fields[2].parse().map_err(|_| anyhow!("Invalid query start: {}", fields[2]))?;
        let query_end: GenomicPos = fields[3].parse().map_err(|_| anyhow!("Invalid query end: {}", fields[3]))?;
        
        // Validate query coordinates
        if query_start >= query_end {
            return Err(anyhow!("Invalid query coordinates: start {} >= end {}", query_start, query_end));
        }
        if query_end > query_len {
            return Err(anyhow!("Query end {} exceeds query length {}", query_end, query_len));
        }
        
        let strand = match fields[4].trim() {
            "+" => Strand::Forward,
            "-" => Strand::Reverse,
            _ => return Err(anyhow!("Invalid strand: '{}', expected '+' or '-'", fields[4])),
        };

        // Validate and parse target information
        let target_name = fields[5].trim().to_string();
        if target_name.is_empty() {
            return Err(anyhow!("Target name cannot be empty"));
        }
        
        let target_len: GenomicPos = fields[6].parse().map_err(|_| anyhow!("Invalid target length: {}", fields[6]))?;
        let target_start: GenomicPos = fields[7].parse().map_err(|_| anyhow!("Invalid target start: {}", fields[7]))?;
        let target_end: GenomicPos = fields[8].parse().map_err(|_| anyhow!("Invalid target end: {}", fields[8]))?;
        
        // Validate target coordinates
        if target_start >= target_end {
            return Err(anyhow!("Invalid target coordinates: start {} >= end {}", target_start, target_end));
        }
        if target_end > target_len {
            return Err(anyhow!("Target end {} exceeds target length {}", target_end, target_len));
        }
        
        let residue_matches: u64 = fields[9].parse().map_err(|_| anyhow!("Invalid residue matches: {}", fields[9]))?;
        let alignment_len: u64 = fields[10].parse().map_err(|_| anyhow!("Invalid alignment length: {}", fields[10]))?;
        let mapping_quality: u8 = fields[11].parse().map_err(|_| anyhow!("Invalid mapping quality: {}", fields[11]))?;
        
        // Validate alignment statistics
        if residue_matches > alignment_len {
            return Err(anyhow!("Residue matches {} cannot exceed alignment length {}", residue_matches, alignment_len));
        }

        let mut tags = Vec::new();
        for field in fields.iter().skip(12) {
            let tag_parts: Vec<&str> = field.splitn(3, ':').collect();
            if tag_parts.len() == 3 {
                tags.push((
                    tag_parts[0].to_string(),
                    tag_parts[1].to_string(),
                    tag_parts[2].to_string(),
                ));
            }
        }

        Ok(PafRecord {
            query_name,
            query_len,
            query_start,
            query_end,
            strand,
            target_name,
            target_len,
            target_start,
            target_end,
            residue_matches,
            alignment_len,
            mapping_quality,
            tags,
        })
    }
}

pub struct PafReader {
    reader: BufReader<File>,
}

impl PafReader {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(PafReader { reader })
    }

    pub fn records(self) -> impl Iterator<Item = Result<PafRecord>> {
        self.reader.lines().map(|line| {
            let line = line?;
            if line.trim().is_empty() || line.starts_with('#') {
                // Skip empty lines and comments
                return Ok(None);
            }
            line.parse().map(Some)
        }).filter_map(|result| match result {
            Ok(Some(record)) => Some(Ok(record)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        })
    }
}

pub struct PafWriter {
    writer: BufWriter<File>,
}

impl PafWriter {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        Ok(PafWriter { writer })
    }

    pub fn write_record(&mut self, record: &PafRecord) -> Result<()> {
        writeln!(self.writer, "{}", record.to_line())?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PafStats {
    pub total_records: u64,
    pub total_aligned_length: u64,
    pub mean_identity: f64,
    pub median_identity: f64,
    pub min_identity: f64,
    pub max_identity: f64,
    pub strand_balance: f64, // fraction of forward strand alignments
}

impl PafStats {
    pub fn compute<I>(records: I) -> Result<Self> 
    where
        I: Iterator<Item = Result<PafRecord>>,
    {
        let mut total_records = 0u64;
        let mut total_aligned_length = 0u64;
        let mut identities = Vec::new();
        let mut forward_count = 0u64;

        for record_result in records {
            let record = record_result?;
            total_records += 1;
            total_aligned_length += record.alignment_len;
            
            let identity = record.identity();
            identities.push(identity);
            
            if record.strand == Strand::Forward {
                forward_count += 1;
            }
        }

        if identities.is_empty() {
            return Ok(PafStats {
                total_records: 0,
                total_aligned_length: 0,
                mean_identity: 0.0,
                median_identity: 0.0,
                min_identity: 0.0,
                max_identity: 0.0,
                strand_balance: 0.0,
            });
        }

        identities.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        let mean_identity = identities.iter().sum::<f64>() / identities.len() as f64;
        let median_identity = if identities.len() == 1 {
            identities[0]
        } else {
            // Proper median calculation for both odd and even lengths
            let mid = identities.len() / 2;
            if identities.len() % 2 == 0 {
                (identities[mid - 1] + identities[mid]) / 2.0
            } else {
                identities[mid]
            }
        };
        let min_identity = identities[0];
        let max_identity = identities[identities.len() - 1];
        let strand_balance = forward_count as f64 / total_records as f64;

        Ok(PafStats {
            total_records,
            total_aligned_length,
            mean_identity,
            median_identity,
            min_identity,
            max_identity,
            strand_balance,
        })
    }
}