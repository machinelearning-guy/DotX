use crate::types::*;
use anyhow::{anyhow, Result};
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastaRecord {
    pub header: String,
    pub sequence_start: u64,
    pub sequence_length: GenomicPos,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastaIndex {
    pub path: PathBuf,
    pub sha256: String,
    pub records: Vec<FastaRecord>,
    pub total_length: GenomicPos,
    pub record_map: HashMap<String, usize>,
}

impl FastaIndex {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        
        // Check file size before mapping to prevent issues with empty files
        let metadata = file.metadata()?;
        if metadata.len() == 0 {
            return Err(anyhow!("FASTA file is empty: {}", path.display()));
        }
        
        // Safe memory mapping with proper error handling
        let mmap = unsafe { 
            Mmap::map(&file).map_err(|e| {
                anyhow!("Failed to memory map FASTA file {}: {}", path.display(), e)
            })?
        };
        
        let mut records = Vec::new();
        let mut record_map = HashMap::new();
        let mut total_length = 0;
        
        let mut current_header = String::new();
        let mut sequence_start = 0u64;
        let mut sequence_length = 0u64;
        let mut position = 0u64;
        let mut in_header = false;
        let mut line_start = 0u64;
        
        for (i, &byte) in mmap.iter().enumerate() {
            let pos = i as u64;
            
            match byte {
                b'>' => {
                    // Save previous record if exists
                    if !current_header.is_empty() {
                        let record = FastaRecord {
                            header: current_header.clone(),
                            sequence_start,
                            sequence_length,
                        };
                        record_map.insert(extract_sequence_name(&current_header), records.len());
                        records.push(record);
                        total_length += sequence_length;
                    }
                    
                    in_header = true;
                    line_start = pos + 1;
                    current_header.clear();
                    sequence_length = 0;
                }
                b'\n' | b'\r' => {
                    if in_header {
                        // End of header line with safe slice bounds
                        let start_idx = line_start as usize;
                        let end_idx = pos as usize;
                        if start_idx < mmap.len() && end_idx <= mmap.len() && start_idx <= end_idx {
                            current_header = String::from_utf8_lossy(&mmap[start_idx..end_idx]).to_string();
                        } else {
                            return Err(anyhow!("Invalid header bounds in FASTA file"));
                        }
                        in_header = false;
                        sequence_start = pos + 1;
                        
                        // Skip any additional newlines with safe bounds checking
                        let mut next_pos = pos + 1;
                        while next_pos < mmap.len() as u64 {
                            match mmap.get(next_pos as usize) {
                                Some(&b'\n') | Some(&b'\r') => next_pos += 1,
                                _ => break,
                            }
                        }
                        sequence_start = next_pos;
                    }
                    line_start = pos + 1;
                }
                _ => {
                    if !in_header && byte != b'\n' && byte != b'\r' && byte != b' ' && byte != b'\t' {
                        sequence_length += 1;
                    }
                }
            }
        }
        
        // Save last record
        if !current_header.is_empty() {
            let record = FastaRecord {
                header: current_header.clone(),
                sequence_start,
                sequence_length,
            };
            record_map.insert(extract_sequence_name(&current_header), records.len());
            records.push(record);
            total_length += sequence_length;
        }
        
        let sha256 = calculate_sha256(&mmap);
        
        Ok(FastaIndex {
            path,
            sha256,
            records,
            total_length,
            record_map,
        })
    }

    pub fn get_record(&self, name: &str) -> Option<&FastaRecord> {
        self.record_map.get(name).and_then(|&idx| self.records.get(idx))
    }

    pub fn to_genome_info(&self) -> GenomeInfo {
        let mut genome = GenomeInfo::new();
        
        for record in &self.records {
            let name = extract_sequence_name(&record.header);
            genome.add_contig(name, record.sequence_length);
        }
        
        genome
    }

    pub fn extract_sequence(&self, name: &str, start: GenomicPos, length: GenomicPos) -> Result<Vec<u8>> {
        let record = self.get_record(name).ok_or_else(|| anyhow!("Sequence not found: {}", name))?;
        
        if start + length > record.sequence_length {
            return Err(anyhow!("Region out of bounds for sequence {}", name));
        }
        
        let file = File::open(&self.path)?;
        
        // Safe memory mapping with error handling
        let mmap = unsafe { 
            Mmap::map(&file).map_err(|e| {
                anyhow!("Failed to memory map FASTA file {}: {}", self.path.display(), e)
            })?
        };
        
        let mut sequence = Vec::with_capacity(length as usize);
        let mut collected = 0u64;
        let mut pos = record.sequence_start;
        let mut skipped = 0u64;
        
        while collected < length && pos < mmap.len() as u64 {
            // Safe array access with bounds checking
            let byte = match mmap.get(pos as usize) {
                Some(&b) => b,
                None => break, // Reached end of file unexpectedly
            };
            
            if byte != b'\n' && byte != b'\r' && byte != b' ' && byte != b'\t' {
                if skipped >= start {
                    sequence.push(byte.to_ascii_uppercase());
                    collected += 1;
                } else {
                    skipped += 1;
                }
            }
            pos += 1;
        }
        
        Ok(sequence)
    }
}

fn extract_sequence_name(header: &str) -> String {
    header.split_whitespace().next().unwrap_or(header).to_string()
}

fn calculate_sha256(data: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Validation functions for FASTA files
pub struct FastaValidator;

impl FastaValidator {
    /// Validate FASTA file format and return detailed validation results
    pub fn validate_file<P: AsRef<Path>>(path: P) -> Result<ValidationResult> {
        let path = path.as_ref();
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        
        if metadata.len() == 0 {
            return Ok(ValidationResult {
                is_valid: false,
                errors: vec!["File is empty".to_string()],
                warnings: Vec::new(),
                sequence_count: 0,
                total_length: 0,
            });
        }
        
        let mmap = unsafe { 
            Mmap::map(&file).map_err(|e| {
                anyhow!("Failed to memory map file {}: {}", path.display(), e)
            })?
        };
        
        Self::validate_content(&mmap)
    }
    
    /// Validate FASTA content from memory-mapped data
    pub fn validate_content(data: &[u8]) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut sequence_count = 0;
        let mut total_length = 0;
        let mut current_header = String::new();
        let mut current_seq_len = 0;
        let mut in_header = false;
        let mut line_number = 1;
        let mut has_sequences = false;
        
        // Check if file starts with a header
        if !data.starts_with(b">") {
            errors.push("FASTA file must start with a header line (>)".to_string());
        }
        
        for (i, &byte) in data.iter().enumerate() {
            match byte {
                b'>' => {
                    // Save previous sequence if exists
                    if !current_header.is_empty() {
                        if current_seq_len == 0 {
                            warnings.push(format!("Sequence '{}' has no bases", current_header));
                        } else {
                            sequence_count += 1;
                            total_length += current_seq_len;
                            has_sequences = true;
                        }
                    }
                    
                    in_header = true;
                    current_header.clear();
                    current_seq_len = 0;
                }
                b'\n' => {
                    line_number += 1;
                    if in_header {
                        in_header = false;
                        if current_header.trim().is_empty() {
                            errors.push(format!("Empty header at line {}", line_number - 1));
                        }
                    }
                }
                b'\r' => {
                    // Handle Windows line endings
                    if in_header {
                        in_header = false;
                        if current_header.trim().is_empty() {
                            errors.push(format!("Empty header at line {}", line_number));
                        }
                    }
                }
                _ => {
                    if in_header {
                        current_header.push(byte as char);
                    } else {
                        // Validate sequence characters
                        match byte {
                            b'A' | b'T' | b'G' | b'C' | b'N' |
                            b'a' | b't' | b'g' | b'c' | b'n' |
                            b'R' | b'Y' | b'S' | b'W' | b'K' | b'M' |
                            b'r' | b'y' | b's' | b'w' | b'k' | b'm' |
                            b'B' | b'D' | b'H' | b'V' |
                            b'b' | b'd' | b'h' | b'v' |
                            b' ' | b'\t' => {
                                if byte != b' ' && byte != b'\t' {
                                    current_seq_len += 1;
                                }
                            }
                            _ => {
                                warnings.push(format!("Invalid nucleotide '{}' at position {} (line {})", 
                                                     byte as char, i, line_number));
                            }
                        }
                    }
                }
            }
        }
        
        // Process final sequence
        if !current_header.is_empty() {
            if current_seq_len == 0 {
                warnings.push(format!("Sequence '{}' has no bases", current_header));
            } else {
                sequence_count += 1;
                total_length += current_seq_len;
                has_sequences = true;
            }
        }
        
        if !has_sequences {
            errors.push("No valid sequences found in file".to_string());
        }
        
        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            sequence_count,
            total_length,
        })
    }
    
    /// Quick validation for format checking (faster, less detailed)
    pub fn is_valid_format<P: AsRef<Path>>(path: P) -> bool {
        match Self::validate_file(path) {
            Ok(result) => result.is_valid,
            Err(_) => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub sequence_count: u64,
    pub total_length: u64,
}