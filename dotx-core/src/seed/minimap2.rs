//! Minimap2 wrapper for seeding
//!
//! This module provides a subprocess interface to the minimap2 binary,
//! supporting all minimap2 presets and parsing the output into Anchors.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::collections::HashMap;
use tempfile::NamedTempFile;

use crate::types::{Anchor, Strand};
use super::{AlgorithmParams, SeedParams, SeedResult, SeedError, Seeder};

/// Minimap2 seeding engine
pub struct Minimap2Seeder {
    binary_path: String,
}

impl Minimap2Seeder {
    pub fn new() -> Self {
        Self {
            binary_path: "minimap2".to_string(),
        }
    }

    /// Create seeder with custom binary path
    pub fn with_binary_path(binary_path: String) -> Self {
        Self { binary_path }
    }

    /// Check if minimap2 binary is available
    fn check_binary_available(&self) -> bool {
        Command::new(&self.binary_path)
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// Write sequence to temporary FASTA file
    fn write_fasta_file(sequence: &[u8], seq_id: &str) -> SeedResult<NamedTempFile> {
        let mut temp_file = NamedTempFile::new()
            .map_err(|e| SeedError::Io(e))?;

        writeln!(temp_file, ">{}", seq_id)?;
        
        // Write sequence in 80-character lines
        for chunk in sequence.chunks(80) {
            temp_file.write_all(chunk)?;
            temp_file.write_all(b"\n")?;
        }
        
        temp_file.flush()?;
        Ok(temp_file)
    }

    /// Parse PAF line into an Anchor
    fn parse_paf_line(line: &str) -> SeedResult<Option<Anchor>> {
        let fields: Vec<&str> = line.trim().split('\t').collect();
        
        if fields.len() < 12 {
            return Err(SeedError::Parse(format!("Invalid PAF line: insufficient fields")));
        }

        let query_id = fields[0].to_string();
        let query_len = fields[1].parse::<u64>()
            .map_err(|_| SeedError::Parse("Invalid query length".to_string()))?;
        let query_start = fields[2].parse::<u64>()
            .map_err(|_| SeedError::Parse("Invalid query start".to_string()))?;
        let query_end = fields[3].parse::<u64>()
            .map_err(|_| SeedError::Parse("Invalid query end".to_string()))?;
        
        let strand_char = fields[4];
        let strand = match strand_char {
            "+" => Strand::Forward,
            "-" => Strand::Reverse,
            _ => return Err(SeedError::Parse(format!("Invalid strand: {}", strand_char))),
        };
        
        let target_id = fields[5].to_string();
        let target_len = fields[6].parse::<u64>()
            .map_err(|_| SeedError::Parse("Invalid target length".to_string()))?;
        let target_start = fields[7].parse::<u64>()
            .map_err(|_| SeedError::Parse("Invalid target start".to_string()))?;
        let target_end = fields[8].parse::<u64>()
            .map_err(|_| SeedError::Parse("Invalid target end".to_string()))?;
        
        let residue_matches = fields[9].parse::<u32>()
            .map_err(|_| SeedError::Parse("Invalid residue matches".to_string()))?;
        let alignment_block_length = fields[10].parse::<u64>()
            .map_err(|_| SeedError::Parse("Invalid alignment block length".to_string()))?;
        let mapq = fields[11].parse::<u8>()
            .map_err(|_| SeedError::Parse("Invalid mapping quality".to_string()))?;

        // Calculate identity from residue matches and alignment length
        let identity = if alignment_block_length > 0 {
            Some((residue_matches as f32 / alignment_block_length as f32) * 100.0)
        } else {
            None
        };

        let mut anchor = Anchor::new(
            query_id,
            target_id,
            query_start,
            query_end,
            target_start,
            target_end,
            strand,
            "minimap2".to_string(),
        );
        
        anchor.mapq = Some(mapq);
        anchor.identity = identity;
        anchor.query_length = Some(query_len);
        anchor.target_length = Some(target_len);
        anchor.alignment_block_length = Some(alignment_block_length);
        anchor.residue_matches = Some(residue_matches);

        Ok(Some(anchor))
    }

    /// Build minimap2 command arguments
    fn build_command_args(&self, params: &SeedParams) -> Vec<String> {
        let mut args = Vec::new();
        
        match &params.algorithm_params {
            AlgorithmParams::Minimap2 { preset, extra_args } => {
                if !preset.is_empty() {
                    args.push("-x".to_string());
                    args.push(preset.clone());
                }
                
                // Add k-mer size if specified
                if params.k != 15 { // 15 is default for most presets
                    args.push("-k".to_string());
                    args.push(params.k.to_string());
                }
                
                // Add frequency filtering if specified
                if let Some(max_freq) = params.max_freq {
                    args.push("-f".to_string());
                    args.push(max_freq.to_string());
                }

                // Add minimum anchor length filtering
                if params.min_anchor_len > 0 {
                    args.push("-m".to_string());
                    args.push(params.min_anchor_len.to_string());
                }
                
                // Add extra user-specified arguments
                args.extend(extra_args.clone());
            }
            _ => {
                return vec![];
            }
        }
        
        // Always output PAF format
        args.push("-c".to_string()); // Output CIGAR in PAF
        
        args
    }
}

impl Seeder for Minimap2Seeder {
    fn seed(
        &self,
        query: &[u8],
        query_id: &str,
        target: &[u8],
        target_id: &str,
        params: &SeedParams,
    ) -> SeedResult<Vec<Anchor>> {
        if !self.is_available() {
            return Err(SeedError::ExternalTool(
                format!("minimap2 binary not found at: {}", self.binary_path)
            ));
        }

        // Create temporary FASTA files
        let query_file = Self::write_fasta_file(query, query_id)?;
        let target_file = Self::write_fasta_file(target, target_id)?;

        // Build command arguments
        let mut cmd_args = self.build_command_args(params);
        cmd_args.push(target_file.path().to_string_lossy().to_string());
        cmd_args.push(query_file.path().to_string_lossy().to_string());

        // Execute minimap2
        let mut child = Command::new(&self.binary_path)
            .args(&cmd_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| SeedError::ExternalTool(format!("Failed to start minimap2: {}", e)))?;

        let stdout = child.stdout.take()
            .ok_or_else(|| SeedError::ExternalTool("Failed to capture stdout".to_string()))?;
        
        let reader = BufReader::new(stdout);
        let mut anchors = Vec::new();

        // Parse PAF output
        for line in reader.lines() {
            let line = line.map_err(|e| SeedError::Io(e))?;
            
            if line.trim().is_empty() || line.starts_with('#') {
                continue; // Skip empty lines and comments
            }

            match Self::parse_paf_line(&line)? {
                Some(anchor) => anchors.push(anchor),
                None => continue,
            }
        }

        // Wait for process to finish
        let status = child.wait()
            .map_err(|e| SeedError::ExternalTool(format!("Failed to wait for minimap2: {}", e)))?;

        if !status.success() {
            let stderr = std::process::Stdio::piped();
            return Err(SeedError::ExternalTool(
                format!("minimap2 failed with exit code: {:?}", status.code())
            ));
        }

        Ok(anchors)
    }

    fn name(&self) -> &'static str {
        "minimap2"
    }

    fn is_available(&self) -> bool {
        self.check_binary_available()
    }
}

impl Default for Minimap2Seeder {
    fn default() -> Self {
        Self::new()
    }
}

/// Common minimap2 presets
pub struct Minimap2Presets;

impl Minimap2Presets {
    /// Assembly-to-assembly alignment (asm5)
    pub fn asm5() -> AlgorithmParams {
        AlgorithmParams::Minimap2 {
            preset: "asm5".to_string(),
            extra_args: vec![],
        }
    }

    /// Assembly-to-assembly alignment (asm10)
    pub fn asm10() -> AlgorithmParams {
        AlgorithmParams::Minimap2 {
            preset: "asm10".to_string(),
            extra_args: vec![],
        }
    }

    /// Oxford Nanopore reads to reference (map-ont)
    pub fn map_ont() -> AlgorithmParams {
        AlgorithmParams::Minimap2 {
            preset: "map-ont".to_string(),
            extra_args: vec![],
        }
    }

    /// PacBio reads to reference (map-pb)
    pub fn map_pb() -> AlgorithmParams {
        AlgorithmParams::Minimap2 {
            preset: "map-pb".to_string(),
            extra_args: vec![],
        }
    }

    /// Short reads to reference (sr)
    pub fn sr() -> AlgorithmParams {
        AlgorithmParams::Minimap2 {
            preset: "sr".to_string(),
            extra_args: vec![],
        }
    }

    /// Splice-aware alignment (splice)
    pub fn splice() -> AlgorithmParams {
        AlgorithmParams::Minimap2 {
            preset: "splice".to_string(),
            extra_args: vec![],
        }
    }

    /// Custom preset with additional arguments
    pub fn custom(preset: &str, extra_args: Vec<String>) -> AlgorithmParams {
        AlgorithmParams::Minimap2 {
            preset: preset.to_string(),
            extra_args,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_QUERY: &[u8] = b"ATCGATCGATCGATCGATCGATCGATCGATCG";
    const TEST_TARGET: &[u8] = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";

    #[test]
    fn test_minimap2_seeder_creation() {
        let seeder = Minimap2Seeder::new();
        assert_eq!(seeder.name(), "minimap2");
        assert_eq!(seeder.binary_path, "minimap2");
    }

    #[test]
    fn test_minimap2_with_custom_path() {
        let seeder = Minimap2Seeder::with_binary_path("/custom/path/minimap2".to_string());
        assert_eq!(seeder.binary_path, "/custom/path/minimap2");
    }

    #[test]
    fn test_paf_line_parsing() {
        let paf_line = "query1\t100\t0\t50\t+\ttarget1\t200\t10\t60\t45\t50\t60";
        let anchor = Minimap2Seeder::parse_paf_line(paf_line).unwrap().unwrap();
        
        assert_eq!(anchor.query_id, "query1");
        assert_eq!(anchor.target_id, "target1");
        assert_eq!(anchor.query_start, 0);
        assert_eq!(anchor.query_end, 50);
        assert_eq!(anchor.target_start, 10);
        assert_eq!(anchor.target_end, 60);
        assert!(matches!(anchor.strand, Strand::Forward));
        assert_eq!(anchor.mapq, Some(60));
        assert!(anchor.identity.is_some());
    }

    #[test]
    fn test_invalid_paf_line() {
        let paf_line = "incomplete\tline";
        let result = Minimap2Seeder::parse_paf_line(paf_line);
        assert!(result.is_err());
    }

    #[test]
    fn test_command_args_building() {
        let seeder = Minimap2Seeder::new();
        let params = SeedParams {
            k: 19,
            algorithm_params: Minimap2Presets::asm5(),
            max_freq: Some(100),
            min_anchor_len: 100,
            ..Default::default()
        };
        
        let args = seeder.build_command_args(&params);
        assert!(args.contains(&"-x".to_string()));
        assert!(args.contains(&"asm5".to_string()));
        assert!(args.contains(&"-k".to_string()));
        assert!(args.contains(&"19".to_string()));
        assert!(args.contains(&"-f".to_string()));
        assert!(args.contains(&"100".to_string()));
        assert!(args.contains(&"-c".to_string()));
    }

    #[test]
    fn test_presets() {
        match Minimap2Presets::asm5() {
            AlgorithmParams::Minimap2 { preset, .. } => {
                assert_eq!(preset, "asm5");
            }
            _ => panic!("Expected Minimap2 algorithm params"),
        }

        match Minimap2Presets::map_ont() {
            AlgorithmParams::Minimap2 { preset, .. } => {
                assert_eq!(preset, "map-ont");
            }
            _ => panic!("Expected Minimap2 algorithm params"),
        }
    }

    #[test]
    fn test_write_fasta_file() {
        let sequence = b"ATCGATCG";
        let seq_id = "test_seq";
        
        let temp_file = Minimap2Seeder::write_fasta_file(sequence, seq_id).unwrap();
        
        // Read back the file to verify contents
        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.starts_with(">test_seq\n"));
        assert!(content.contains("ATCGATCG"));
    }

    // Integration test - only runs if minimap2 is available
    #[test]
    fn test_minimap2_integration() {
        let seeder = Minimap2Seeder::new();
        
        // Skip test if minimap2 not available
        if !seeder.is_available() {
            return;
        }

        let params = SeedParams {
            algorithm_params: Minimap2Presets::asm5(),
            ..Default::default()
        };

        let result = seeder.seed(
            TEST_QUERY,
            "query",
            TEST_TARGET,
            "target",
            &params,
        );

        // Should not error even if no matches found
        assert!(result.is_ok());
    }
}
