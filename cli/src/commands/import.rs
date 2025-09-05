//! Import command implementation - convert PAF/MAF/MUMmer/SAM to .dotxdb format

use anyhow::{Result, Context, anyhow};
use std::path::PathBuf;

use crate::config::Config;
use crate::FormatType;

pub fn execute(
    config: &Config,
    input: PathBuf,
    format: Option<FormatType>,
    db: PathBuf,
    build_tiles: bool,
    reference: Option<PathBuf>,
    query: Option<PathBuf>,
    compression: u8,
) -> Result<()> {
    log::info!("Starting import to .dotxdb format");
    log::info!("Input file: {}", input.display());
    log::info!("Output database: {}", db.display());
    
    // Validate input file exists
    if !input.exists() {
        return Err(anyhow!("Input file does not exist: {}", input.display()));
    }
    
    // Auto-detect format if not specified
    let detected_format = match format {
        Some(fmt) => {
            log::info!("Using user-specified format: {:?}", fmt);
            fmt
        }
        None => {
            let fmt = detect_format(&input);
            log::info!(
                "Auto-detected format: {:?} (use --format to override)",
                fmt
            );
            fmt
        }
    };
    
    // Validate compression level
    if compression > 9 {
        return Err(anyhow!("Compression level must be between 0 and 9, got: {}", compression));
    }
    
    // Load and parse input file
    log::info!("Parsing input file");
    let anchors = parse_input_file(&input, &detected_format, reference.as_ref(), query.as_ref())
        .context("Failed to parse input file")?;
    
    log::info!("Parsed {} anchors", anchors.len());
    
    // Build .dotxdb file (optionally with tiles)
    log::info!("Building .dotxdb database{}",
        if build_tiles { " with tiles" } else { "" });
    build_dotxdb(&anchors, &db, compression, config, build_tiles)
        .context("Failed to build .dotxdb database")?;
    
    log::info!("Import completed successfully");
    log::info!("Database written to: {}", db.display());
    
    Ok(())
}

fn detect_format(path: &PathBuf) -> FormatType {
    if let Some(extension) = path.extension() {
        match extension.to_string_lossy().to_lowercase().as_str() {
            "paf" => FormatType::Paf,
            "maf" => FormatType::Maf,
            "sam" => FormatType::Sam,
            "bam" => FormatType::Bam,
            "delta" | "coords" => FormatType::Mummer,
            _ => {
                log::warn!("Unknown file extension, defaulting to PAF format");
                FormatType::Paf
            }
        }
    } else {
        log::warn!("No file extension found, defaulting to PAF format");
        FormatType::Paf
    }
}

fn parse_input_file(
    path: &PathBuf,
    _format: &FormatType,
    _reference: Option<&PathBuf>,
    _query: Option<&PathBuf>,
) -> Result<Vec<dotx_core::types::Anchor>> {
    dotx_core::io::parse_alignment_file(path)
}

fn build_dotxdb(
    anchors: &[dotx_core::types::Anchor],
    output: &PathBuf,
    _compression: u8,
    _config: &Config,
    with_tiles: bool,
) -> Result<()> {
    use dotx_core::store::DotXStore;
    use dotx_core::{build_density_tiles, TileBuildConfig};

    let mut store = DotXStore::new();

    // Collect contig metadata if present
    use std::collections::HashMap;
    let mut q_max: HashMap<String, u64> = HashMap::new();
    let mut t_max: HashMap<String, u64> = HashMap::new();

    for a in anchors.iter() {
        if let Some(qlen) = a.query_length {
            q_max.entry(a.q.clone()).and_modify(|m| *m = (*m).max(qlen)).or_insert(qlen);
        }
        if let Some(tlen) = a.target_length {
            t_max.entry(a.t.clone()).and_modify(|m| *m = (*m).max(tlen)).or_insert(tlen);
        }
    }
    for (name, len) in q_max.into_iter() {
        store.add_query_contig(name, len, None);
    }
    for (name, len) in t_max.into_iter() {
        store.add_target_contig(name, len, None);
    }

    if with_tiles {
        let tiles = build_density_tiles(anchors, TileBuildConfig::default());
        store.write_to_file_with_tiles(output, anchors, &tiles)
            .context("Failed to write .dotxdb file with tiles")?;
    } else {
        store.write_to_file(output, anchors)
            .context("Failed to write .dotxdb file")?;
    }
    Ok(())
}

// Placeholder: a future implementation will compute and write tile indices to the store

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_detect_format() {
        assert!(matches!(detect_format(&PathBuf::from("test.paf")), FormatType::Paf));
        assert!(matches!(detect_format(&PathBuf::from("test.maf")), FormatType::Maf));
        assert!(matches!(detect_format(&PathBuf::from("test.sam")), FormatType::Sam));
        assert!(matches!(detect_format(&PathBuf::from("test.bam")), FormatType::Bam));
        assert!(matches!(detect_format(&PathBuf::from("test.delta")), FormatType::Mummer));
        assert!(matches!(detect_format(&PathBuf::from("test.coords")), FormatType::Mummer));
        assert!(matches!(detect_format(&PathBuf::from("test.unknown")), FormatType::Paf)); // Default fallback
    }
    
    #[test]
    fn test_compression_validation() {
        let config = Config::default();
        let temp_input = NamedTempFile::new().unwrap();
        let temp_db = NamedTempFile::new().unwrap();
        
        // Write minimal PAF content
        writeln!(temp_input.as_file(), "query\t100\t0\t50\t+\ttarget\t200\t10\t60\t40\t50\t60").unwrap();
        temp_input.as_file().sync_all().unwrap();
        
        // Test invalid compression level
        let result = execute(
            &config,
            temp_input.path().to_path_buf(),
            Some(FormatType::Paf),
            temp_db.path().to_path_buf(),
            false,
            None,
            None,
            15, // Invalid compression level
        );
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Compression level must be between 0 and 9"));
    }
}
