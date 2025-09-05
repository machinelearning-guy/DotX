//! Refine command implementation - compute exact alignments on ROI tiles

use anyhow::{Result, Context, anyhow};
use std::path::PathBuf;
use std::collections::HashMap;

use crate::config::Config;
use crate::{RefineEngine, DeviceType};

#[allow(clippy::too_many_arguments)]
pub fn execute(
    config: &Config,
    db: PathBuf,
    roi: Option<String>,
    engine: RefineEngine,
    device: DeviceType,
    batch_size: Option<u32>,
    max_align_len: Option<u32>,
    reference: Option<PathBuf>,
    query: Option<PathBuf>,
) -> Result<()> {
    log::info!("Starting exact alignment refinement");
    log::info!("Database: {}", db.display());
    log::info!("Engine: {:?}, Device: {:?}", engine, device);
    
    // Validate database exists
    if !db.exists() {
        return Err(anyhow!("Database file does not exist: {}", db.display()));
    }
    
    // Parse ROI specification if provided
    let roi_regions = if let Some(roi_str) = roi {
        Some(parse_roi_specification(&roi_str)?)
    } else {
        None
    };
    
    if let Some(ref regions) = roi_regions {
        log::info!("Processing {} ROI regions", regions.len());
    } else {
        log::info!("No ROI specified, using default tile policy");
    }
    
    // Validate sequence files if provided
    if let Some(ref ref_path) = reference {
        if !ref_path.exists() {
            return Err(anyhow!("Reference sequence file does not exist: {}", ref_path.display()));
        }
    }
    
    if let Some(ref qry_path) = query {
        if !qry_path.exists() {
            return Err(anyhow!("Query sequence file does not exist: {}", qry_path.display()));
        }
    }
    
    // Build refinement configuration
    let refine_config = RefineConfig {
        engine,
        device,
        batch_size: batch_size.unwrap_or(config.verify.batch_size),
        max_align_len: max_align_len.unwrap_or(config.verify.max_align_len),
        roi_regions,
    };
    
    log::info!("Refinement config: batch_size={}, max_align_len={}", 
              refine_config.batch_size, refine_config.max_align_len);
    
    // Load database
    log::info!("Loading database for refinement");
    let mut database = load_database_for_refinement(&db)
        .context("Failed to load database")?;
    
    // Load sequence data if provided
    let sequences = load_sequences(reference.as_ref(), query.as_ref())
        .context("Failed to load sequences")?;
    
    // Select tiles/anchors to refine
    log::info!("Selecting anchors for refinement");
    let refinement_targets = select_refinement_targets(&database, &refine_config)
        .context("Failed to select refinement targets")?;
    
    log::info!("Selected {} anchors/tiles for exact alignment", refinement_targets.len());
    
    if refinement_targets.is_empty() {
        log::info!("No anchors selected for refinement, nothing to do");
        return Ok(());
    }
    
    // Initialize alignment engine
    log::info!("Initializing alignment engine");
    let aligner = create_alignment_engine(&refine_config)
        .context("Failed to create alignment engine")?;
    
    // Process refinements
    log::info!("Processing exact alignments");
    let refinement_results = process_refinements(
        &refinement_targets,
        &sequences,
        &aligner,
        &refine_config,
    ).context("Failed to process refinements")?;
    
    log::info!("Completed {} exact alignments", refinement_results.len());
    
    // Update database with refinement results
    log::info!("Updating database with refinement results");
    update_database_with_results(&mut database, refinement_results, &db)
        .context("Failed to update database")?;
    
    log::info!("Refinement completed successfully");
    
    Ok(())
}

#[derive(Debug)]
struct RefineConfig {
    engine: RefineEngine,
    device: DeviceType,
    batch_size: u32,
    max_align_len: u32,
    roi_regions: Option<Vec<RoiRegion>>,
}

#[derive(Debug, Clone)]
struct RoiRegion {
    contig: String,
    start: u64,
    end: u64,
}

#[derive(Debug)]
struct RefinementDatabase {
    store: dotx_core::store::DotXStore,
    anchors: Vec<dotx_core::types::Anchor>,
}

#[derive(Debug)]
struct SequenceData {
    reference_sequences: HashMap<String, Vec<u8>>,
    query_sequences: HashMap<String, Vec<u8>>,
}

#[derive(Debug)]
struct RefinementTarget {
    anchor: dotx_core::types::Anchor,
    priority: f32, // Higher values indicate higher priority for refinement
}

#[derive(Debug)]
struct AlignmentResult {
    anchor: dotx_core::types::Anchor,
    identity: f32,
    alignment_score: i32,
    edit_distance: u32,
    alignment_length: u32,
    cigar: Option<String>, // CIGAR string if available
}

trait AlignmentEngine {
    fn align_batch(&self, targets: &[RefinementTarget], sequences: &SequenceData) -> Result<Vec<AlignmentResult>>;
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
}

struct WfaAligner {
    device: DeviceType,
}

struct EdlibAligner {
    device: DeviceType,
}

impl AlignmentEngine for WfaAligner {
    fn align_batch(&self, targets: &[RefinementTarget], sequences: &SequenceData) -> Result<Vec<AlignmentResult>> {
        log::debug!("Running WFA alignment on {} targets", targets.len());
        
        let mut results = Vec::new();
        
        for target in targets {
            // Get sequence data for this alignment
            let query_seq = sequences.query_sequences.get(&target.anchor.q)
                .ok_or_else(|| anyhow!("Query sequence not found: {}", target.anchor.q))?;
            let target_seq = sequences.reference_sequences.get(&target.anchor.t)
                .ok_or_else(|| anyhow!("Target sequence not found: {}", target.anchor.t))?;
            
            // Extract subsequences for alignment
            let query_subseq = extract_sequence_region(
                query_seq,
                target.anchor.qs,
                target.anchor.qe,
            )?;
            let target_subseq = extract_sequence_region(
                target_seq,
                target.anchor.ts,
                target.anchor.te,
            )?;
            
            // Perform alignment (placeholder implementation)
            let result = perform_wfa_alignment(&query_subseq, &target_subseq, self.device)?;
            
            results.push(AlignmentResult {
                anchor: target.anchor.clone(),
                identity: result.identity,
                alignment_score: result.score,
                edit_distance: result.edit_distance,
                alignment_length: result.alignment_length,
                cigar: result.cigar,
            });
        }
        
        Ok(results)
    }
    
    fn name(&self) -> &str {
        "WFA"
    }
    
    fn is_available(&self) -> bool {
        // In real implementation, check if WFA library is available
        match self.device {
            DeviceType::Cpu => true,  // CPU version usually available
            DeviceType::Gpu => false, // GPU version may not be available
        }
    }
}

impl AlignmentEngine for EdlibAligner {
    fn align_batch(&self, targets: &[RefinementTarget], sequences: &SequenceData) -> Result<Vec<AlignmentResult>> {
        log::debug!("Running Edlib alignment on {} targets", targets.len());
        
        let mut results = Vec::new();
        
        for target in targets {
            // Similar to WFA implementation but using Edlib
            let query_seq = sequences.query_sequences.get(&target.anchor.q)
                .ok_or_else(|| anyhow!("Query sequence not found: {}", target.anchor.q))?;
            let target_seq = sequences.reference_sequences.get(&target.anchor.t)
                .ok_or_else(|| anyhow!("Target sequence not found: {}", target.anchor.t))?;
            
            let query_subseq = extract_sequence_region(
                query_seq,
                target.anchor.qs,
                target.anchor.qe,
            )?;
            let target_subseq = extract_sequence_region(
                target_seq,
                target.anchor.ts,
                target.anchor.te,
            )?;
            
            let result = perform_edlib_alignment(&query_subseq, &target_subseq)?;
            
            results.push(AlignmentResult {
                anchor: target.anchor.clone(),
                identity: result.identity,
                alignment_score: result.score,
                edit_distance: result.edit_distance,
                alignment_length: result.alignment_length,
                cigar: result.cigar,
            });
        }
        
        Ok(results)
    }
    
    fn name(&self) -> &str {
        "Edlib"
    }
    
    fn is_available(&self) -> bool {
        true // Edlib is usually available
    }
}

#[derive(Debug)]
struct AlignmentOutput {
    identity: f32,
    score: i32,
    edit_distance: u32,
    alignment_length: u32,
    cigar: Option<String>,
}

fn parse_roi_specification(roi: &str) -> Result<Vec<RoiRegion>> {
    let mut regions = Vec::new();
    
    for region_str in roi.split(',') {
        let region_str = region_str.trim();
        if region_str.is_empty() {
            continue;
        }
        
        let region = parse_single_roi_region(region_str)?;
        regions.push(region);
    }
    
    if regions.is_empty() {
        return Err(anyhow!("No valid ROI regions found in specification: {}", roi));
    }
    
    Ok(regions)
}

fn parse_single_roi_region(region: &str) -> Result<RoiRegion> {
    // Parse formats like "chr1:12.3M-18.6M"
    let parts: Vec<&str> = region.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid ROI format: {}. Expected 'contig:start-end'", region));
    }
    
    let contig = parts[0].to_string();
    let range_parts: Vec<&str> = parts[1].split('-').collect();
    if range_parts.len() != 2 {
        return Err(anyhow!("Invalid ROI range: {}. Expected 'start-end'", parts[1]));
    }
    
    let start = parse_genomic_position(range_parts[0])?;
    let end = parse_genomic_position(range_parts[1])?;
    
    if start >= end {
        return Err(anyhow!("ROI start ({}) must be less than end ({})", start, end));
    }
    
    Ok(RoiRegion { contig, start, end })
}

fn parse_genomic_position(pos: &str) -> Result<u64> {
    let pos = pos.trim().to_uppercase();
    
    if pos.ends_with('K') {
        let num: f64 = pos.trim_end_matches('K').parse()
            .context("Invalid number format")?;
        Ok((num * 1_000.0) as u64)
    } else if pos.ends_with('M') {
        let num: f64 = pos.trim_end_matches('M').parse()
            .context("Invalid number format")?;
        Ok((num * 1_000_000.0) as u64)
    } else if pos.ends_with('G') {
        let num: f64 = pos.trim_end_matches('G').parse()
            .context("Invalid number format")?;
        Ok((num * 1_000_000_000.0) as u64)
    } else {
        pos.parse::<u64>().context("Invalid number format")
    }
}

fn load_database_for_refinement(path: &PathBuf) -> Result<RefinementDatabase> {
    use std::fs::File;
    use std::io::Seek;
    use dotx_core::store::DotXStore;

    log::debug!("Loading database for refinement from {}", path.display());

    let store = DotXStore::read_from_file(path)
        .with_context(|| format!("Failed to open .dotxdb: {}", path.display()))?;

    let mut file = File::open(path)
        .with_context(|| format!("Failed to open .dotxdb to read anchors: {}", path.display()))?;
    let anchors = store
        .read_anchors(&mut file)
        .context("Failed to read anchors from .dotxdb")?;

    Ok(RefinementDatabase { store, anchors })
}

fn load_sequences(
    reference: Option<&PathBuf>,
    query: Option<&PathBuf>,
) -> Result<SequenceData> {
    use dotx_core::io::fasta::FastaParser;
    
    let mut reference_sequences = HashMap::new();
    let mut query_sequences = HashMap::new();
    
    if let Some(ref_path) = reference {
        log::debug!("Loading reference sequences from {}", ref_path.display());
        let sequences = FastaParser::parse_file(ref_path)?;
        for sequence in sequences.into_iter() {
            reference_sequences.insert(sequence.id.clone(), sequence.data);
        }
        
        log::info!("Loaded {} reference sequences", reference_sequences.len());
    }
    
    if let Some(qry_path) = query {
        log::debug!("Loading query sequences from {}", qry_path.display());
        let sequences = FastaParser::parse_file(qry_path)?;
        for sequence in sequences.into_iter() {
            query_sequences.insert(sequence.id.clone(), sequence.data);
        }
        
        log::info!("Loaded {} query sequences", query_sequences.len());
    }
    
    Ok(SequenceData {
        reference_sequences,
        query_sequences,
    })
}

fn select_refinement_targets(
    database: &RefinementDatabase,
    config: &RefineConfig,
) -> Result<Vec<RefinementTarget>> {
    let mut targets = Vec::new();
    
    for anchor in &database.anchors {
        // Check if anchor is within ROI regions
        let in_roi = if let Some(ref regions) = config.roi_regions {
            regions.iter().any(|region| {
                // Check if anchor overlaps with ROI region
                (anchor.t == region.contig && anchor.ts < region.end && anchor.te > region.start) ||
                (anchor.q == region.contig && anchor.qs < region.end && anchor.qe > region.start)
            })
        } else {
            true // No ROI specified, include all anchors
        };
        
        if !in_roi {
            continue;
        }
        
        // Check if anchor length is within limits
        let anchor_len = anchor.alignment_length().max(anchor.query_span_length()).max(anchor.target_span_length());
        if anchor_len > config.max_align_len as u64 {
            log::debug!("Skipping anchor with length {} > max {}", anchor_len, config.max_align_len);
            continue;
        }
        
        // Calculate priority (higher for longer anchors, lower identity if available)
        let priority = if let Some(identity) = anchor.identity {
            // Lower identity gets higher priority for refinement
            anchor_len as f32 * (1.0 - identity / 100.0)
        } else {
            anchor_len as f32 // No identity info, prioritize by length
        };
        
        targets.push(RefinementTarget {
            anchor: anchor.clone(),
            priority,
        });
    }
    
    // Sort by priority (highest first)
    targets.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
    
    Ok(targets)
}

fn create_alignment_engine(config: &RefineConfig) -> Result<Box<dyn AlignmentEngine>> {
    let engine: Box<dyn AlignmentEngine> = match config.engine {
        RefineEngine::Wfa => Box::new(WfaAligner { device: config.device }),
        RefineEngine::Edlib => Box::new(EdlibAligner { device: config.device }),
        RefineEngine::Custom => {
            return Err(anyhow!("Custom alignment engine is not yet implemented"));
        }
    };
    
    if !engine.is_available() {
        return Err(anyhow!("Alignment engine {} is not available for device {:?}", 
                          engine.name(), config.device));
    }
    
    log::info!("Using {} alignment engine on {:?}", engine.name(), config.device);
    Ok(engine)
}

fn process_refinements(
    targets: &[RefinementTarget],
    sequences: &SequenceData,
    aligner: &Box<dyn AlignmentEngine>,
    config: &RefineConfig,
) -> Result<Vec<AlignmentResult>> {
    let mut all_results = Vec::new();
    
    // Process in batches
    for batch in targets.chunks(config.batch_size as usize) {
        log::debug!("Processing batch of {} alignments", batch.len());
        
        let batch_results = aligner.align_batch(batch, sequences)
            .context("Failed to process alignment batch")?;
        
        all_results.extend(batch_results);
    }
    
    Ok(all_results)
}

fn update_database_with_results(
    database: &mut RefinementDatabase,
    results: Vec<AlignmentResult>,
    db_path: &PathBuf,
) -> Result<()> {
    use std::fs;
    use tempfile::NamedTempFile;

    log::debug!("Updating database with {} refinement results", results.len());

    // Index refinement identities by a tuple key to match anchors robustly
    use std::collections::HashMap;
    let mut id_map: HashMap<(String, String, u64, u64, u64, u64, dotx_core::types::Strand), (f32, i32, u32, u32)> = HashMap::new();
    for r in &results {
        id_map.insert(
            (
                r.anchor.q.clone(),
                r.anchor.t.clone(),
                r.anchor.qs,
                r.anchor.qe,
                r.anchor.ts,
                r.anchor.te,
                r.anchor.strand,
            ),
            // Store identity as percentage to match Anchor semantics
            (r.identity * 100.0, r.alignment_score, r.edit_distance, r.alignment_length),
        );
    }

    // Update anchors in-memory
    let mut updated = 0usize;
    for a in database.anchors.iter_mut() {
        if let Some((id_pct, _score, _ed, _alen)) = id_map.get(&(
            a.q.clone(),
            a.t.clone(),
            a.qs,
            a.qe,
            a.ts,
            a.te,
            a.strand,
        )) {
            a.identity = Some(*id_pct);
            updated += 1;
        }
    }

    log::info!("Updated identity for {} anchors", updated);

    // Prepare verify records: map alignment results to tile IDs when tiles are present
    let (existing_tiles, existing_verify, verify_records) = {
        use std::fs::File;
        use std::io::Seek;
        use dotx_core::store::VerifyResult as VR;

        // Open a fresh store handle from disk to read tiles/verify
        let store_for_read = dotx_core::store::DotXStore::read_from_file(db_path)
            .with_context(|| format!("Failed to open DB for tiles/verify: {}", db_path.display()))?;
        let mut f = File::open(db_path)
            .with_context(|| format!("Failed to reopen DB to read tiles/verify: {}", db_path.display()))?;
        let tiles = store_for_read.read_tiles(&mut f).unwrap_or_default();
        // Read existing verify records (if any)
        let verify_existing = store_for_read.read_verify(&mut f).unwrap_or_default();

        // Compute extents for mapping anchors to tiles
        let (t_min, t_max, q_min, q_max) = compute_extents(&database.anchors);
        let choose_level = |tiles: &[dotx_core::tiles::DensityTile]| -> Option<u8> {
            if tiles.is_empty() { return None; }
            let mut lv: Vec<u8> = tiles.iter().map(|t| t.level).collect();
            lv.sort_unstable(); lv.dedup();
            lv.into_iter().max()
        };
        let level_opt = choose_level(&tiles);
        let (res_x, res_y) = level_opt
            .map(|lvl| infer_level_resolution(&tiles, lvl))
            .unwrap_or((0, 0));

        // Build new verify records if tiles exist
        let mut vrs: Vec<VR> = Vec::new();
        if let Some(level) = level_opt {
            for r in &results {
                let (ix, iy) = world_to_tile(&r.anchor, t_min, t_max, q_min, q_max, res_x, res_y);
                let tile_id = pack_tile_id(level, ix, iy);
                vrs.push(VR {
                    tile_id,
                    identity: r.identity * 100.0, // store as percentage
                    insertions: 0,
                    deletions: 0,
                    substitutions: r.edit_distance, // placeholder mapping
                });
            }
        }
        (tiles, verify_existing, vrs)
    };

    let temp = NamedTempFile::new().context("Failed to create temp file for DB rewrite")?;
    let temp_path = temp.path().to_path_buf();
    // Merge existing verify records with new ones (by tile_id, new overwrites)
    let merged_verify = if existing_tiles.is_empty() {
        Vec::new()
    } else {
        merge_verify(existing_verify, verify_records)
    };

    // Rewrite the database file atomically with updated anchors, preserving tiles and verify
    let mut store = std::mem::replace(&mut database.store, dotx_core::store::DotXStore::new());
    if existing_tiles.is_empty() {
        store
            .write_to_file(&temp_path, &database.anchors)
            .context("Failed to write updated .dotxdb")?;
    } else if merged_verify.is_empty() {
        store
            .write_to_file_with_tiles(&temp_path, &database.anchors, &existing_tiles)
            .context("Failed to write updated .dotxdb with tiles")?;
    } else {
        store
            .write_to_file_with_tiles_and_verify(&temp_path, &database.anchors, &existing_tiles, &merged_verify)
            .context("Failed to write updated .dotxdb with tiles+verify")?;
    }

    // Replace original file
    fs::rename(&temp_path, db_path)
        .with_context(|| format!("Failed to replace original DB at {}", db_path.display()))?;

    log::info!("Database updated with refinement identities: {}", db_path.display());
    Ok(())
}

fn infer_level_resolution(tiles: &[dotx_core::tiles::DensityTile], level: u8) -> (u32, u32) {
    let mut max_x = 0u32; let mut max_y = 0u32;
    for t in tiles.iter().filter(|t| t.level == level) {
        if t.x > max_x { max_x = t.x; }
        if t.y > max_y { max_y = t.y; }
    }
    (max_x + 1, max_y + 1)
}

fn compute_extents(anchors: &[dotx_core::types::Anchor]) -> (u64, u64, u64, u64) {
    let mut t_min = u64::MAX; let mut t_max = 0u64; let mut q_min = u64::MAX; let mut q_max = 0u64;
    for a in anchors { t_min = t_min.min(a.ts); t_max = t_max.max(a.te); q_min = q_min.min(a.qs); q_max = q_max.max(a.qe); }
    if t_min == u64::MAX { t_min = 0; }
    if q_min == u64::MAX { q_min = 0; }
    (t_min, t_max, q_min, q_max)
}

fn world_to_tile(
    anchor: &dotx_core::types::Anchor,
    t_min: u64, t_max: u64,
    q_min: u64, q_max: u64,
    res_x: u32, res_y: u32,
) -> (u32, u32) {
    let t_span = (t_max - t_min).max(1) as f64;
    let q_span = (q_max - q_min).max(1) as f64;
    let x_norm = ((anchor.ts.saturating_sub(t_min)) as f64 / t_span).clamp(0.0, 1.0);
    let y_norm = ((anchor.qs.saturating_sub(q_min)) as f64 / q_span).clamp(0.0, 1.0);
    let mut ix = (x_norm * res_x.saturating_sub(1) as f64).floor() as i64;
    let mut iy = (y_norm * res_y.saturating_sub(1) as f64).floor() as i64;
    if ix < 0 { ix = 0; }
    if iy < 0 { iy = 0; }
    (ix as u32, iy as u32)
}

fn pack_tile_id(level: u8, x: u32, y: u32) -> u64 {
    ((level as u64) << 56) | ((x as u64) << 28) | (y as u64)
}

fn merge_verify(
    existing: Vec<dotx_core::store::VerifyResult>,
    new_items: Vec<dotx_core::store::VerifyResult>,
) -> Vec<dotx_core::store::VerifyResult> {
    use std::collections::HashMap;
    let mut map: HashMap<u64, dotx_core::store::VerifyResult> = HashMap::new();
    for v in existing { map.insert(v.tile_id, v); }
    for v in new_items { map.insert(v.tile_id, v); }
    let mut out: Vec<_> = map.into_values().collect();
    out.sort_by_key(|v| v.tile_id);
    out
}

fn extract_sequence_region(sequence: &[u8], start: u64, end: u64) -> Result<Vec<u8>> {
    let start = start as usize;
    let end = end as usize;
    
    if start >= sequence.len() {
        return Err(anyhow!("Start position {} is beyond sequence length {}", start, sequence.len()));
    }
    
    let end = end.min(sequence.len());
    Ok(sequence[start..end].to_vec())
}

fn perform_wfa_alignment(query: &[u8], target: &[u8], device: DeviceType) -> Result<AlignmentOutput> {
    // Placeholder WFA alignment implementation
    log::trace!("WFA alignment: query={} bp, target={} bp, device={:?}", 
               query.len(), target.len(), device);
    
    // In real implementation, this would use the WFA library
    // to perform exact alignment with gap-affine penalties
    
    let alignment_length = query.len().min(target.len()) as u32;
    let edit_distance = (query.len().abs_diff(target.len()) / 10) as u32; // Placeholder
    let identity = 1.0 - (edit_distance as f32 / alignment_length as f32);
    
    Ok(AlignmentOutput {
        identity,
        score: alignment_length as i32 - edit_distance as i32 * 2,
        edit_distance,
        alignment_length,
        cigar: None, // Would generate proper CIGAR string
    })
}

fn perform_edlib_alignment(query: &[u8], target: &[u8]) -> Result<AlignmentOutput> {
    // Placeholder Edlib alignment implementation
    log::trace!("Edlib alignment: query={} bp, target={} bp", query.len(), target.len());
    
    // In real implementation, this would use the Edlib library
    // for fast edit distance computation
    
    let alignment_length = query.len().max(target.len()) as u32;
    let edit_distance = (query.len().abs_diff(target.len()) / 8) as u32; // Placeholder
    let identity = 1.0 - (edit_distance as f32 / alignment_length as f32);
    
    Ok(AlignmentOutput {
        identity,
        score: -(edit_distance as i32),
        edit_distance,
        alignment_length,
        cigar: None, // Would generate proper CIGAR string
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_genomic_position() -> Result<()> {
        assert_eq!(parse_genomic_position("1000")?, 1000);
        assert_eq!(parse_genomic_position("12.3K")?, 12300);
        assert_eq!(parse_genomic_position("18.6M")?, 18600000);
        assert_eq!(parse_genomic_position("2.1G")?, 2100000000);
        
        assert!(parse_genomic_position("invalid").is_err());
        
        Ok(())
    }
    
    #[test]
    fn test_parse_single_roi_region() -> Result<()> {
        let region = parse_single_roi_region("chr1:12.3M-18.6M")?;
        assert_eq!(region.contig, "chr1");
        assert_eq!(region.start, 12300000);
        assert_eq!(region.end, 18600000);
        
        assert!(parse_single_roi_region("invalid").is_err());
        assert!(parse_single_roi_region("chr1:100M-50M").is_err()); // start > end
        
        Ok(())
    }
    
    #[test]
    fn test_parse_roi_specification() -> Result<()> {
        let regions = parse_roi_specification("chr1:1M-2M,chr2:500K-1.5M")?;
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].contig, "chr1");
        assert_eq!(regions[1].contig, "chr2");
        
        Ok(())
    }
    
    #[test]
    fn test_extract_sequence_region() -> Result<()> {
        let sequence = b"ATCGATCGATCG";
        let region = extract_sequence_region(sequence, 3, 8)?;
        assert_eq!(region, b"GATCG");
        
        // Test bounds checking
        let region = extract_sequence_region(sequence, 10, 20)?;
        assert_eq!(region, b"CG"); // Trimmed to sequence length
        
        assert!(extract_sequence_region(sequence, 20, 25).is_err()); // Start beyond sequence
        
        Ok(())
    }
}
