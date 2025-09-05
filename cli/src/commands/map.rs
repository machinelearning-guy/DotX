//! Map command implementation - align sequences with various engines

use anyhow::{Result, Context, anyhow};
use std::path::PathBuf;
use dotx_core::seed::{
    Seeder, SeederFactory, SeedParams, AlgorithmParams,
};
use dotx_core::io::fasta::FastaParser;
use dotx_core::types::{Sequence, Anchor, Strand};

use crate::config::Config;
use crate::{EngineType};

#[allow(clippy::too_many_arguments)]
pub fn execute(
    config: &Config,
    deterministic: bool,
    reference: PathBuf,
    query: PathBuf,
    output: PathBuf,
    engine: EngineType,
    preset: Option<String>,
    k: Option<u32>,
    syncmer_s: Option<u32>,
    syncmer_t: Option<u32>,
    strobemer_window: Option<u32>,
    max_freq: Option<u32>,
    min_anchor_len: Option<u32>,
    mask_low_complexity: bool,
    extra_args: Vec<String>,
) -> Result<()> {
    log::info!("Starting sequence alignment with {:?} engine", engine);
    log::info!("Reference: {}", reference.display());
    log::info!("Query: {}", query.display());
    log::info!("Output: {}", output.display());
    
    // Set deterministic seed if requested
    if deterministic {
        log::info!("Running in deterministic mode");
        // TODO: Set deterministic seeds for reproducible results
    }
    
    // Read reference and query sequences
    log::info!("Loading reference sequence");
    let reference_sequences = load_fasta(&reference)
        .context("Failed to load reference sequences")?;
    
    log::info!("Loading query sequences");  
    let query_sequences = load_fasta(&query)
        .context("Failed to load query sequences")?;
    
    log::info!("Loaded {} reference sequences and {} query sequences", 
              reference_sequences.len(), query_sequences.len());
    
    // Build seeding parameters
    let mut seed_params = build_seed_params(
        config,
        &engine,
        preset,
        k,
        syncmer_s,
        syncmer_t,
        strobemer_window,
        max_freq,
        min_anchor_len,
        mask_low_complexity,
        extra_args,
    )?;

    // Apply deterministic seed if requested either via CLI or config
    if deterministic || config.general.deterministic {
        // Chosen fixed seed for reproducibility; could be made configurable later
        seed_params.deterministic_seed = Some(42);
    }
    
    log::info!("Seeding parameters: k={}, algorithm={:?}", 
              seed_params.k, seed_params.algorithm_params);
    
    // Create seeder
    let seeder = SeederFactory::create(&seed_params);
    
    // Check if seeder is available (e.g., external tools)
    if !seeder.is_available() {
        return Err(anyhow!("Seeding engine {:?} is not available. Please check that required external tools are installed.", engine));
    }
    
    // Generate alignments
    log::info!("Generating seed alignments");
    let mut all_anchors = Vec::new();
    let mut anchor_count = 0;
    
    // Process all-vs-all combinations
    for ref_seq in &reference_sequences {
        for query_seq in &query_sequences {
            log::debug!("Processing {} vs {}", ref_seq.id, query_seq.id);
            
            let anchors = seeder.seed(
                &query_seq.data,
                &query_seq.id,
                &ref_seq.data,
                &ref_seq.id,
                &seed_params,
            ).with_context(|| format!("Failed to generate seeds for {} vs {}", query_seq.id, ref_seq.id))?;
            
            all_anchors.extend(anchors.into_iter());
            anchor_count = all_anchors.len();
        }
    }
    
    log::info!("Generated {} total anchors", anchor_count);
    
    // Write PAF output
    log::info!("Writing PAF output to {}", output.display());
    write_paf(&all_anchors, &output)
        .context("Failed to write PAF output")?;
    
    log::info!("Alignment completed successfully");
    Ok(())
}

fn load_fasta(path: &PathBuf) -> Result<Vec<Sequence>> {
    let sequences = FastaParser::parse_file(path)
        .with_context(|| format!("Failed to parse FASTA/FASTQ file: {}", path.display()))?;
    if sequences.is_empty() {
        return Err(anyhow!("No sequences found in file: {}", path.display()));
    }
    Ok(sequences)
}

#[allow(clippy::too_many_arguments)]
fn build_seed_params(
    config: &Config,
    engine: &EngineType,
    preset: Option<String>,
    k: Option<u32>,
    syncmer_s: Option<u32>,
    syncmer_t: Option<u32>,
    strobemer_window: Option<u32>,
    max_freq: Option<u32>,
    min_anchor_len: Option<u32>,
    mask_low_complexity: bool,
    extra_args: Vec<String>,
) -> Result<SeedParams> {
    // Use CLI args, then config, then defaults
    let k_value = k.unwrap_or(config.map.k);
    let max_freq_value = max_freq.unwrap_or(config.map.max_freq);
    let min_anchor_len_value = min_anchor_len.unwrap_or(config.map.min_anchor_len);
    
    let algorithm_params = match engine {
        EngineType::Minimap2 => {
            let preset_str = preset.unwrap_or_else(|| config.map.preset.clone());
            AlgorithmParams::Minimap2 {
                preset: preset_str,
                extra_args,
            }
        }
        EngineType::Syncmer => {
            let s = syncmer_s.unwrap_or(config.map.syncmer.s);
            let t = syncmer_t.unwrap_or(config.map.syncmer.t);
            AlgorithmParams::Syncmer { s, t }
        }
        EngineType::Strobemer => {
            let window_size = strobemer_window.unwrap_or(config.map.strobemer.window_size);
            AlgorithmParams::Strobemer {
                window_size,
                max_distance: config.map.strobemer.max_distance,
                n_strobes: config.map.strobemer.n_strobes,
            }
        }
        EngineType::Kmer => AlgorithmParams::Kmer,
    };
    
    Ok(SeedParams {
        k: k_value,
        algorithm_params,
        max_freq: Some(max_freq_value),
        mask_low_complexity,
        min_anchor_len: min_anchor_len_value,
        deterministic_seed: None,
    })
}

// Seed anchors already use the unified Anchor type; no conversion needed.

fn write_paf(anchors: &[Anchor], output: &PathBuf) -> Result<()> {
    use std::io::Write;
    
    let file = std::fs::File::create(output)
        .with_context(|| format!("Failed to create output file: {}", output.display()))?;
    let mut writer = std::io::BufWriter::new(file);
    
    for anchor in anchors {
        // Write PAF format line
        // PAF format: query_name query_length query_start query_end strand target_name target_length target_start target_end residue_matches alignment_block_length mapping_quality
        let line = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            anchor.q,                                    // query name
            anchor.query_length.unwrap_or(0),           // query length
            anchor.qs,                                   // query start
            anchor.qe,                                   // query end
            anchor.strand,                               // strand
            anchor.t,                                    // target name
            anchor.target_length.unwrap_or(0),          // target length
            anchor.ts,                                   // target start
            anchor.te,                                   // target end
            anchor.residue_matches.unwrap_or(0),        // residue matches
            anchor.alignment_block_length.unwrap_or(0), // alignment block length
            anchor.mapq.unwrap_or(255),                  // mapping quality
        );
        
        writer.write_all(line.as_bytes())?;
    }
    
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;
    
    #[test]
    fn test_build_seed_params() -> Result<()> {
        let config = Config::default();
        
        let params = build_seed_params(
            &config,
            &EngineType::Kmer,
            None,
            Some(21),
            None,
            None,
            None,
            None,
            None,
            true,
            vec![],
        )?;
        
        assert_eq!(params.k, 21);
        assert!(matches!(params.algorithm_params, AlgorithmParams::Kmer));
        assert!(params.mask_low_complexity);
        
        Ok(())
    }
    
    #[test]
    fn test_write_paf() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path().to_path_buf();
        
        let anchor = Anchor::from_parser(
            "query1".to_string(),
            1000,
            10,
            100,
            Strand::Forward,
            "target1".to_string(),
            2000,
            500,
            590,
            80,
            90,
        );
        
        write_paf(&[anchor], &temp_path)?;
        
        let content = std::fs::read_to_string(&temp_path)?;
        assert!(content.contains("query1"));
        assert!(content.contains("target1"));
        assert!(content.contains("1000"));
        assert!(content.contains("2000"));
        
        Ok(())
    }
}
