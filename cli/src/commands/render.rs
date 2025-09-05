//! Render command implementation - export plots to SVG/PNG/PDF with configuration options

use anyhow::{Result, Context, anyhow};
use std::path::PathBuf;

use crate::config::Config;
use crate::{RenderFormat, FlipMode};
use dotx_gpu::vector_export::{VectorExporter, ExportConfig};
use dotx_gpu::{Viewport, LodLevel, RenderStyle};
use dotx_core::types::Anchor;
use dotx_core::store::VerifyResult;

#[allow(clippy::too_many_arguments)]
pub fn execute(
    config: &Config,
    db: PathBuf,
    output: PathBuf,
    format: Option<RenderFormat>,
    strand: String,
    flip: FlipMode,
    theme: String,
    dpi: u32,
    width: Option<u32>,
    height: Option<u32>,
    region: Option<String>,
    legend: bool,
    scale_bar: bool,
) -> Result<()> {
    log::info!("Starting plot rendering");
    log::info!("Input database: {}", db.display());
    log::info!("Output file: {}", output.display());
    
    // Validate input database exists
    if !db.exists() {
        return Err(anyhow!("Database file does not exist: {}", db.display()));
    }
    
    // Auto-detect output format if not specified
    let render_format = format.unwrap_or_else(|| detect_render_format(&output));
    log::info!("Output format: {:?}", render_format);
    
    // Parse strand filter
    let strand_config = parse_strand_filter(&strand)?;
    log::info!("Strand filter: show_plus={}, show_minus={}", 
              strand_config.show_plus, strand_config.show_minus);
    
    // Parse region filter if provided
    let region_filter = if let Some(region_str) = region {
        Some(parse_region_filter(&region_str)?)
    } else {
        None
    };
    
    // Build render configuration
    let render_config = RenderConfig {
        format: render_format,
        strand: strand_config,
        flip,
        theme,
        dpi,
        width: width.unwrap_or(config.render.width),
        height: height.unwrap_or(config.render.height),
        region: region_filter,
        legend,
        scale_bar,
    };
    
    // Load database
    log::info!("Loading database");
    let database = load_database(&db)
        .context("Failed to load .dotxdb database")?;
    
    log::info!("Loaded database with {} anchors", database.anchor_count());
    
    // Apply filters
    log::info!("Applying filters");
    let filtered_anchors = apply_filters(&database, &render_config)
        .context("Failed to apply filters")?;
    
    log::info!("Filtered to {} anchors for rendering", filtered_anchors.len());
    
    // Prepare anchors (apply flips/swaps) and viewport
    let (mut anchors, extents) = transform_for_render(&filtered_anchors, &render_config);
    // Deterministic ordering for reproducible exports
    anchors.sort_by(|a, b| a.t.cmp(&b.t)
        .then(a.q.cmp(&b.q))
        .then(a.ts.cmp(&b.ts))
        .then(a.qs.cmp(&b.qs))
    );
    let viewport = make_viewport_from_extents(extents, render_config.width, render_config.height);
    let lod = determine_lod(&anchors, &viewport);

    let export_cfg = ExportConfig {
        width: render_config.width,
        height: render_config.height,
        dpi: render_config.dpi,
        show_legend: render_config.legend,
        show_scale_bar: render_config.scale_bar,
        show_axes: true,
        show_footer: true,
        show_grid: true,
        title: Some(format!("DOTx — {}", render_config.theme)),
        background_color: "#ffffff".to_string(),
        forward_color: "#2a6fef".to_string(),
        reverse_color: "#e53935".to_string(),
        font_family: "Arial, sans-serif".to_string(),
        font_size: 12,
        provenance_comment: Some(build_provenance_comment(
            &db,
            &output,
            &render_config,
            &viewport,
            lod,
            filtered_anchors.len(),
        )),
    };
    // Build shared RenderStyle for parity with GUI
    let style = RenderStyle {
        show_plus: render_config.strand.show_plus,
        show_minus: render_config.strand.show_minus,
        flip: match render_config.flip {
            FlipMode::None => "none",
            FlipMode::X => "x",
            FlipMode::Y => "y",
            FlipMode::Xy => "xy",
            FlipMode::Rcx => "rcx",
            FlipMode::Rcy => "rcy",
            FlipMode::Rcxy => "rcxy",
        }.to_string(),
        theme: render_config.theme.clone(),
        legend: render_config.legend,
        scale_bar: render_config.scale_bar,
    };
    let exporter = VectorExporter::new(export_cfg).with_style(style);

    // Render plot via dotx-gpu vector exporter
    log::info!("Rendering plot");
    match render_config.format {
        RenderFormat::Svg => {
            if matches!(lod, LodLevel::Overview) && database.tiles().len() > 0 && render_config.region.is_none() {
                exporter.export_svg_overview_tiles(&output, database.tiles(), &viewport)
                    .context("Failed to render SVG (tiles)")?;
            } else {
                let verify_opt = if database.verify().is_empty() { None } else { Some(database.verify()) };
                exporter.export_svg(&output, &anchors, &viewport, lod, verify_opt)
                    .context("Failed to render SVG")?;
            }
        }
        RenderFormat::Png => {
            if matches!(lod, LodLevel::Overview) && database.tiles().len() > 0 && render_config.region.is_none() {
                exporter.export_png_overview_tiles(&output, database.tiles(), &viewport)
                    .context("Failed to render PNG (tiles)")?;
            } else {
                let verify_opt = if database.verify().is_empty() { None } else { Some(database.verify()) };
                exporter.export_png_simple(&output, &anchors, &viewport, lod, verify_opt)
                    .context("Failed to render PNG")?;
            }
        }
        RenderFormat::Pdf => {
            let verify_opt = if database.verify().is_empty() { None } else { Some(database.verify()) };
            exporter.export_pdf(&output, &anchors, &viewport, lod, verify_opt)
                .context("Failed to render PDF")?;
        }
    }
    
    log::info!("Rendering completed successfully");
    log::info!("Output written to: {}", output.display());
    
    Ok(())
}

#[derive(Debug)]
struct RenderConfig {
    format: RenderFormat,
    strand: StrandConfig,
    flip: FlipMode,
    theme: String,
    dpi: u32,
    width: u32,
    height: u32,
    region: Option<RegionFilter>,
    legend: bool,
    scale_bar: bool,
}

fn build_provenance_comment(
    db_path: &PathBuf,
    out_path: &PathBuf,
    cfg: &RenderConfig,
    vp: &Viewport,
    lod: LodLevel,
    n_anchors: usize,
) -> String {
    // Basic provenance for reproducibility
    let version = env!("CARGO_PKG_VERSION");
    let region_str = cfg
        .region
        .as_ref()
        .map(|r| {
            r.regions
                .iter()
                .map(|g| format!("{}:{}-{}", g.contig, g.start, g.end))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_else(|| "(none)".to_string());
    format!(
        "DOTx provenance\n \
         tool: dotx v{version}\n \
         db: {db}\n \
         out: {out}\n \
         width: {w}px, height: {h}px, dpi: {dpi}\n \
         strand: plus={sp}, minus={sm}\n \
         flip: {flip:?}, theme: {theme}\n \
         region: {region}\n \
         viewport: t=({tx0:.0}-{tx1:.0}), q=({qy0:.0}-{qy1:.0})\n \
         lod: {lod:?}, anchorsRendered: {n}",
        version = version,
        db = db_path.display(),
        out = out_path.display(),
        w = cfg.width,
        h = cfg.height,
        dpi = cfg.dpi,
        sp = cfg.strand.show_plus,
        sm = cfg.strand.show_minus,
        flip = cfg.flip,
        theme = cfg.theme,
        region = region_str,
        tx0 = vp.x_min,
        tx1 = vp.x_max,
        qy0 = vp.y_min,
        qy1 = vp.y_max,
        lod = lod,
        n = n_anchors,
    )
}

#[derive(Debug)]
struct StrandConfig {
    show_plus: bool,
    show_minus: bool,
}

#[derive(Debug)]
struct RegionFilter {
    regions: Vec<GenomicRegion>,
}

#[derive(Debug)]
struct GenomicRegion {
    contig: String,
    start: u64,
    end: u64,
}

// Placeholder database type - in real implementation this would use dotx-core
#[derive(Debug)]
struct Database {
    anchors: Vec<dotx_core::types::Anchor>,
    tiles: Vec<dotx_core::tiles::DensityTile>,
    verify: Vec<VerifyResult>,
}

impl Database {
    fn anchor_count(&self) -> usize { self.anchors.len() }
    fn anchors(&self) -> &[dotx_core::types::Anchor] { &self.anchors }
    fn tiles(&self) -> &[dotx_core::tiles::DensityTile] { &self.tiles }
    fn verify(&self) -> &[VerifyResult] { &self.verify }
}

fn detect_render_format(path: &PathBuf) -> RenderFormat {
    if let Some(extension) = path.extension() {
        match extension.to_string_lossy().to_lowercase().as_str() {
            "svg" => RenderFormat::Svg,
            "png" => RenderFormat::Png,
            "pdf" => RenderFormat::Pdf,
            _ => {
                log::warn!("Unknown output format, defaulting to SVG");
                RenderFormat::Svg
            }
        }
    } else {
        log::warn!("No file extension found, defaulting to SVG");
        RenderFormat::Svg
    }
}

fn parse_strand_filter(strand: &str) -> Result<StrandConfig> {
    let strand = strand.replace(" ", ""); // Remove spaces
    
    match strand.as_str() {
        "+" => Ok(StrandConfig { show_plus: true, show_minus: false }),
        "-" => Ok(StrandConfig { show_plus: false, show_minus: true }),
        "+,-" | "-,+" => Ok(StrandConfig { show_plus: true, show_minus: true }),
        "both" | "all" => Ok(StrandConfig { show_plus: true, show_minus: true }),
        "none" => Ok(StrandConfig { show_plus: false, show_minus: false }),
        _ => Err(anyhow!("Invalid strand filter: {}. Use '+', '-', '+,-', 'both', or 'none'", strand))
    }
}

fn parse_region_filter(region: &str) -> Result<RegionFilter> {
    let mut regions = Vec::new();
    
    for region_str in region.split(',') {
        let region_str = region_str.trim();
        if region_str.is_empty() {
            continue;
        }
        
        let parsed_region = parse_single_region(region_str)?;
        regions.push(parsed_region);
    }
    
    if regions.is_empty() {
        return Err(anyhow!("No valid regions found in filter: {}", region));
    }
    
    Ok(RegionFilter { regions })
}

fn parse_single_region(region: &str) -> Result<GenomicRegion> {
    // Parse formats like "chr1:1M-2M" or "contig1:100000-200000"
    let parts: Vec<&str> = region.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid region format: {}. Expected 'contig:start-end'", region));
    }
    
    let contig = parts[0].to_string();
    let range_parts: Vec<&str> = parts[1].split('-').collect();
    if range_parts.len() != 2 {
        return Err(anyhow!("Invalid region range: {}. Expected 'start-end'", parts[1]));
    }
    
    let start = parse_position(range_parts[0])?;
    let end = parse_position(range_parts[1])?;
    
    if start >= end {
        return Err(anyhow!("Region start ({}) must be less than end ({})", start, end));
    }
    
    Ok(GenomicRegion { contig, start, end })
}

fn parse_position(pos: &str) -> Result<u64> {
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

// ----- Rendering helpers (viewport, flips, LOD) -----

fn compute_extents(anchors: &[Anchor]) -> (u64, u64, u64, u64) {
    let mut t_min = u64::MAX; let mut t_max = 0u64; let mut q_min = u64::MAX; let mut q_max = 0u64;
    for a in anchors { t_min = t_min.min(a.ts); t_max = t_max.max(a.te); q_min = q_min.min(a.qs); q_max = q_max.max(a.qe); }
    if t_min == u64::MAX { t_min = 0; }
    if q_min == u64::MAX { q_min = 0; }
    (t_min, t_max, q_min, q_max)
}

fn transform_for_render(anchors: &[Anchor], cfg: &RenderConfig) -> (Vec<Anchor>, (u64,u64,u64,u64)) {
    let (t_min, t_max, q_min, q_max) = compute_extents(anchors);
    let mut out = Vec::with_capacity(anchors.len());
    for a in anchors.iter().cloned() {
        let mut a2 = a.clone();
        match cfg.flip {
            FlipMode::None => {}
            FlipMode::X => {
                let nts = t_max - (a2.ts - t_min);
                let nte = t_max - (a2.te - t_min);
                a2.ts = nts.min(nte); a2.te = nts.max(nte);
            }
            FlipMode::Y => {
                let nqs = q_max - (a2.qs - q_min);
                let nqe = q_max - (a2.qe - q_min);
                a2.qs = nqs.min(nqe); a2.qe = nqs.max(nqe);
            }
            FlipMode::Xy => {
                std::mem::swap(&mut a2.q, &mut a2.t);
                std::mem::swap(&mut a2.qs, &mut a2.ts);
                std::mem::swap(&mut a2.qe, &mut a2.te);
                std::mem::swap(&mut a2.query_length, &mut a2.target_length);
            }
            FlipMode::Rcx => {
                if let Some(tlen) = a2.target_length { let nts = tlen.saturating_sub(a2.ts); let nte = tlen.saturating_sub(a2.te); a2.ts = nts.min(nte); a2.te = nts.max(nte); }
            }
            FlipMode::Rcy => {
                if let Some(qlen) = a2.query_length { let nqs = qlen.saturating_sub(a2.qs); let nqe = qlen.saturating_sub(a2.qe); a2.qs = nqs.min(nqe); a2.qe = nqs.max(nqe); }
            }
            FlipMode::Rcxy => {
                if let Some(tlen) = a2.target_length { let nts = tlen.saturating_sub(a2.ts); let nte = tlen.saturating_sub(a2.te); a2.ts = nts.min(nte); a2.te = nts.max(nte); }
                if let Some(qlen) = a2.query_length { let nqs = qlen.saturating_sub(a2.qs); let nqe = qlen.saturating_sub(a2.qe); a2.qs = nqs.min(nqe); a2.qe = nqs.max(nqe); }
            }
        }
        out.push(a2);
    }
    let ext = compute_extents(&out);
    (out, ext)
}

fn make_viewport_from_extents(ext: (u64,u64,u64,u64), width: u32, height: u32) -> Viewport {
    let (t_min, t_max, q_min, q_max) = ext;
    Viewport::new(t_min as f64, t_max as f64, q_min as f64, q_max as f64, width, height)
}

fn determine_lod(anchors: &[Anchor], vp: &Viewport) -> LodLevel {
    let n = anchors.len();
    if n > 5_000_000 { return LodLevel::Overview; }
    if n > (vp.width as usize * 3) { return LodLevel::MidZoom; }
    LodLevel::DeepZoom
}

fn load_database(path: &PathBuf) -> Result<Database> {
    log::debug!("Loading .dotxdb file from {}", path.display());
    use std::fs::File;
    use std::io::Seek;
    use dotx_core::store::DotXStore;

    let store = DotXStore::read_from_file(path)
        .with_context(|| format!("Failed to open .dotxdb: {}", path.display()))?;

    let mut file = File::open(path)
        .with_context(|| format!("Failed to open .dotxdb for reading anchors: {}", path.display()))?;
    let anchors = store.read_anchors(&mut file)
        .context("Failed to read anchors from .dotxdb")?;
    let tiles = store.read_tiles(&mut file).unwrap_or_default();
    let verify = store.read_verify(&mut file).unwrap_or_default();

    Ok(Database { anchors, tiles, verify })
}

fn apply_filters(database: &Database, config: &RenderConfig) -> Result<Vec<dotx_core::types::Anchor>> {
    let mut filtered = Vec::new();
    
    for anchor in database.anchors() {
        // Apply strand filter
        let keep_strand = match anchor.strand {
            dotx_core::types::Strand::Forward => config.strand.show_plus,
            dotx_core::types::Strand::Reverse => config.strand.show_minus,
        };
        
        if !keep_strand {
            continue;
        }
        
        // Apply region filter if specified
        if let Some(region_filter) = &config.region {
            let mut in_region = false;
            
            for region in &region_filter.regions {
                // Check if anchor overlaps with any specified region
                if anchor.t == region.contig && 
                   anchor.ts < region.end && anchor.te > region.start {
                    in_region = true;
                    break;
                }
                
                // Also check query regions
                if anchor.q == region.contig &&
                   anchor.qs < region.end && anchor.qe > region.start {
                    in_region = true;
                    break;
                }
            }
            
            if !in_region {
                continue;
            }
        }
        
        filtered.push(anchor.clone());
    }
    
    Ok(filtered)
}

// removed legacy manual renderers in favor of dotx-gpu exporter

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_render_format() {
        assert!(matches!(detect_render_format(&PathBuf::from("plot.svg")), RenderFormat::Svg));
        assert!(matches!(detect_render_format(&PathBuf::from("plot.png")), RenderFormat::Png));
        assert!(matches!(detect_render_format(&PathBuf::from("plot.pdf")), RenderFormat::Pdf));
        assert!(matches!(detect_render_format(&PathBuf::from("plot.unknown")), RenderFormat::Svg));
    }
    
    #[test]
    fn test_parse_strand_filter() -> Result<()> {
        let config = parse_strand_filter("+")?;
        assert!(config.show_plus && !config.show_minus);
        
        let config = parse_strand_filter("-")?;
        assert!(!config.show_plus && config.show_minus);
        
        let config = parse_strand_filter("+,-")?;
        assert!(config.show_plus && config.show_minus);
        
        let config = parse_strand_filter("both")?;
        assert!(config.show_plus && config.show_minus);
        
        assert!(parse_strand_filter("invalid").is_err());
        
        Ok(())
    }
    
    #[test]
    fn test_parse_position() -> Result<()> {
        assert_eq!(parse_position("1000")?, 1000);
        assert_eq!(parse_position("1K")?, 1000);
        assert_eq!(parse_position("1.5K")?, 1500);
        assert_eq!(parse_position("1M")?, 1000000);
        assert_eq!(parse_position("2.5M")?, 2500000);
        assert_eq!(parse_position("1G")?, 1000000000);
        
        Ok(())
    }
    
    #[test]
    fn test_parse_single_region() -> Result<()> {
        let region = parse_single_region("chr1:1M-2M")?;
        assert_eq!(region.contig, "chr1");
        assert_eq!(region.start, 1000000);
        assert_eq!(region.end, 2000000);
        
        let region = parse_single_region("contig1:100-200")?;
        assert_eq!(region.contig, "contig1");
        assert_eq!(region.start, 100);
        assert_eq!(region.end, 200);
        
        assert!(parse_single_region("invalid").is_err());
        assert!(parse_single_region("chr1:200-100").is_err()); // start > end
        
        Ok(())
    }
}
