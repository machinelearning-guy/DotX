use crate::types::*;
use crate::paf::PafRecord;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentParams {
    pub preset: String,
    pub k: Option<u16>,        // k-mer size
    pub w: Option<u16>,        // window size
    pub secondary: Option<bool>, // include secondary alignments
    pub min_occ_floor: Option<u32>, // minimum occurrence floor
    pub custom_args: Vec<String>,
}

impl AlignmentParams {
    pub fn bacterial_fast() -> Self {
        Self {
            preset: "bacterial_fast".to_string(),
            k: Some(15),
            w: Some(5),
            secondary: Some(false),
            min_occ_floor: None,
            custom_args: vec!["-x".to_string(), "ava-ont".to_string()],
        }
    }

    pub fn plant_te() -> Self {
        Self {
            preset: "plant_te".to_string(),
            k: Some(15),
            w: Some(10),
            secondary: None,
            min_occ_floor: Some(100),
            custom_args: vec!["-x".to_string(), "map-ont".to_string()],
        }
    }

    pub fn mammal_hq() -> Self {
        Self {
            preset: "mammal_hq".to_string(),
            k: Some(19),
            w: Some(10),
            secondary: None,
            min_occ_floor: None,
            custom_args: vec!["-x".to_string(), "asm5".to_string()],
        }
    }

    pub fn viral() -> Self {
        Self {
            preset: "viral".to_string(),
            k: Some(15),
            w: Some(5),
            secondary: None,
            min_occ_floor: None,
            custom_args: vec!["-x".to_string(), "ava-ont".to_string()],
        }
    }

    pub fn fungal() -> Self {
        Self {
            preset: "fungal".to_string(),
            k: Some(17),
            w: Some(8),
            secondary: Some(true),
            min_occ_floor: Some(50),
            custom_args: vec!["-x".to_string(), "asm10".to_string()],
        }
    }

    pub fn metagenomic() -> Self {
        Self {
            preset: "metagenomic".to_string(),
            k: Some(13),
            w: Some(6),
            secondary: Some(true),
            min_occ_floor: Some(10),
            custom_args: vec!["-x".to_string(), "ava-ont".to_string(), "--dual=yes".to_string()],
        }
    }

    pub fn plant_genome() -> Self {
        Self {
            preset: "plant_genome".to_string(),
            k: Some(19),
            w: Some(12),
            secondary: None,
            min_occ_floor: Some(200),
            custom_args: vec!["-x".to_string(), "asm20".to_string()],
        }
    }

    pub fn prokaryotic() -> Self {
        Self {
            preset: "prokaryotic".to_string(),
            k: Some(17),
            w: Some(8),
            secondary: Some(false),
            min_occ_floor: None,
            custom_args: vec!["-x".to_string(), "asm5".to_string()],
        }
    }

    pub fn repetitive() -> Self {
        Self {
            preset: "repetitive".to_string(),
            k: Some(15),
            w: Some(12),
            secondary: Some(true),
            min_occ_floor: Some(500),
            custom_args: vec!["-x".to_string(), "asm20".to_string(), "--dual=yes".to_string()],
        }
    }

    pub fn to_minimap2_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        
        // Add custom args first
        args.extend(self.custom_args.iter().cloned());
        
        // Add parameter-specific args
        if let Some(k) = self.k {
            args.push("-k".to_string());
            args.push(k.to_string());
        }
        
        if let Some(w) = self.w {
            args.push("-w".to_string());
            args.push(w.to_string());
        }
        
        if let Some(secondary) = self.secondary {
            if !secondary {
                args.push("--secondary=no".to_string());
            }
        }
        
        if let Some(min_occ_floor) = self.min_occ_floor {
            args.push("--min-occ-floor".to_string());
            args.push(min_occ_floor.to_string());
        }
        
        args
    }
}

pub trait Aligner {
    fn align(&self, reference: &PathBuf, query: &PathBuf, params: &AlignmentParams, output: &PathBuf) -> Result<AlignmentResult>;
    fn name(&self) -> &'static str;
    fn version(&self) -> String;
}

#[derive(Debug, Clone)]
pub struct AlignmentResult {
    pub output_file: PathBuf,
    pub stats: AlignmentRunStats,
    pub runtime_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentRunStats {
    pub total_records: u64,
    pub total_aligned_bases: u64,
    pub mean_identity: f64,
    pub median_identity: f64,
    pub identity_histogram: Vec<u32>, // 100 bins from 0.0 to 1.0
    pub strand_forward_count: u64,
    pub strand_reverse_count: u64,
    pub length_histogram: Vec<u32>, // Log10 binned lengths
}

pub struct Minimap2Aligner {
    binary_path: PathBuf,
}

impl Minimap2Aligner {
    pub fn new(binary_path: Option<PathBuf>) -> Result<Self> {
        let binary_path = binary_path.unwrap_or_else(|| {
            // Try to find minimap2 in PATH or use bundled version
            if let Ok(path) = which::which("minimap2") {
                path
            } else {
                // Use bundled minimap2 (would be included in distribution)
                PathBuf::from("minimap2")
            }
        });
        
        Ok(Self { binary_path })
    }
}

impl Aligner for Minimap2Aligner {
    fn align(&self, reference: &PathBuf, query: &PathBuf, params: &AlignmentParams, output: &PathBuf) -> Result<AlignmentResult> {
        use std::process::Command;
        use std::time::Instant;
        
        let start_time = Instant::now();
        
        let mut cmd = Command::new(&self.binary_path);
        
        // Add parameters
        for arg in params.to_minimap2_args() {
            cmd.arg(arg);
        }
        
        // Add input files and output
        cmd.arg(reference)
           .arg(query)
           .arg("-o")
           .arg(output);
        
        log::info!("Running minimap2: {:?}", cmd);
        
        let output_result = cmd.output()?;
        let runtime_seconds = start_time.elapsed().as_secs_f64();
        
        if !output_result.status.success() {
            let stderr = String::from_utf8_lossy(&output_result.stderr);
            return Err(anyhow::anyhow!("minimap2 failed: {}", stderr));
        }
        
        // Calculate stats from the output PAF file
        let stats = calculate_paf_stats(output)?;
        
        Ok(AlignmentResult {
            output_file: output.clone(),
            stats,
            runtime_seconds,
        })
    }

    fn name(&self) -> &'static str {
        "minimap2"
    }

    fn version(&self) -> String {
        use std::process::Command;
        
        let output = Command::new(&self.binary_path)
            .arg("--version")
            .output()
            .unwrap_or_else(|_| std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: b"unknown".to_vec(),
                stderr: Vec::new(),
            });
        
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}

fn calculate_paf_stats(paf_path: &PathBuf) -> Result<AlignmentRunStats> {
    use crate::paf::PafReader;
    
    let mut reader = PafReader::new(paf_path)?;
    let mut total_records = 0u64;
    let mut total_aligned_bases = 0u64;
    let mut identities = Vec::new();
    let mut strand_forward_count = 0u64;
    let mut strand_reverse_count = 0u64;
    let mut identity_histogram = vec![0u32; 100];
    let mut length_histogram = vec![0u32; 20]; // Log10 from 1 to 10^20

    for record_result in reader.records() {
        let record = record_result?;
        total_records += 1;
        total_aligned_bases += record.alignment_len;
        
        let identity = record.identity();
        identities.push(identity);
        
        // Update identity histogram
        let identity_bin = ((identity * 100.0) as usize).min(99);
        identity_histogram[identity_bin] += 1;
        
        // Update strand counts
        match record.strand {
            Strand::Forward => strand_forward_count += 1,
            Strand::Reverse => strand_reverse_count += 1,
        }
        
        // Update length histogram
        let length = record.alignment_len;
        if length > 0 {
            let log_length = (length as f64).log10() as usize;
            let length_bin = log_length.min(19);
            length_histogram[length_bin] += 1;
        }
    }

    // Calculate mean and median identity
    let mean_identity = if identities.is_empty() {
        0.0
    } else {
        identities.iter().sum::<f64>() / identities.len() as f64
    };

    identities.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_identity = if identities.is_empty() {
        0.0
    } else if identities.len() == 1 {
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

    Ok(AlignmentRunStats {
        total_records,
        total_aligned_bases,
        mean_identity,
        median_identity,
        identity_histogram,
        strand_forward_count,
        strand_reverse_count,
        length_histogram,
    })
}