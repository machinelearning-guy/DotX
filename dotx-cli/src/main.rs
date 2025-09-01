use clap::{Parser, Subcommand};
use dotx_core::*;
use dotx_gpu::*;
use std::path::PathBuf;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "dotx")]
#[command(about = "DotX - Extreme-scale dot plot visualization")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new project and perform alignment
    Align {
        /// Reference FASTA file
        #[arg(short, long)]
        reference: PathBuf,
        
        /// Query FASTA file  
        #[arg(short, long)]
        query: PathBuf,
        
        /// Alignment preset (bacterial, plant_te, mammal, viral)
        #[arg(short, long, default_value = "mammal")]
        preset: String,
        
        /// Output PAF file
        #[arg(short, long)]
        output: PathBuf,
        
        /// Number of threads
        #[arg(short, long, default_value = "4")]
        threads: usize,
    },
    
    /// Build tiles from PAF alignment
    Tile {
        /// Input PAF file
        #[arg(short, long)]
        paf: PathBuf,
        
        /// Output project file
        #[arg(short, long)]
        project: PathBuf,
        
        /// LOD levels to build (e.g., "0..10" or "5,6,7")
        #[arg(short, long, default_value = "0..10")]
        lod: String,
        
        /// Use GPU acceleration
        #[arg(long)]
        gpu: bool,
    },
    
    /// Generate plot from project
    Plot {
        /// Project file
        #[arg(short, long)]
        project: PathBuf,
        
        /// Output image file (SVG, PNG, PDF)
        #[arg(short, long)]
        output: PathBuf,
        
        /// View region (e.g., "chr1:1-50M vs chr2:1-30M")
        #[arg(short, long)]
        view: Option<String>,
        
        /// Image width in pixels
        #[arg(long, default_value = "1200")]
        width: u32,
        
        /// Image height in pixels  
        #[arg(long, default_value = "800")]
        height: u32,
    },
    
    /// Quick comparison (align + tile + plot in one command)
    Quick {
        /// Reference FASTA file
        #[arg(short, long)]
        reference: PathBuf,
        
        /// Query FASTA file
        #[arg(short, long)]
        query: PathBuf,
        
        /// Alignment preset
        #[arg(short, long, default_value = "mammal")]
        preset: String,
        
        /// Output image file
        #[arg(short, long)]
        export: PathBuf,
        
        /// Use GPU acceleration
        #[arg(long)]
        gpu: bool,
    },
    
    /// Project management commands
    Project {
        #[command(subcommand)]
        action: ProjectCommands,
    },
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// Create new project
    New {
        /// Project name
        name: String,
        
        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
    },
    
    /// Show project information
    Info {
        /// Project file
        project: PathBuf,
    },
    
    /// List tiles in project
    ListTiles {
        /// Project file
        project: PathBuf,
        
        /// LOD level filter
        #[arg(short, long)]
        lod: Option<u16>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();
    
    match cli.command {
        Commands::Align { reference, query, preset, output, threads } => {
            cmd_align(reference, query, preset, output, threads)
        }
        Commands::Tile { paf, project, lod, gpu } => {
            cmd_tile(paf, project, lod, gpu)
        }
        Commands::Plot { project, output, view, width, height } => {
            cmd_plot(project, output, view, width, height)
        }
        Commands::Quick { reference, query, preset, export, gpu } => {
            cmd_quick(reference, query, preset, export, gpu)
        }
        Commands::Project { action } => {
            cmd_project(action)
        }
    }
}

fn cmd_align(reference: PathBuf, query: PathBuf, preset: String, output: PathBuf, _threads: usize) -> Result<()> {
    log::info!("Starting alignment: {} vs {}", reference.display(), query.display());
    
    // Get alignment parameters based on preset
    let params = match preset.as_str() {
        "bacterial" => AlignmentParams::bacterial_fast(),
        "plant_te" => AlignmentParams::plant_te(),
        "mammal" => AlignmentParams::mammal_hq(),
        "viral" => AlignmentParams::viral(),
        "fungal" => AlignmentParams::fungal(),
        "metagenomic" => AlignmentParams::metagenomic(),
        "plant_genome" => AlignmentParams::plant_genome(),
        "prokaryotic" => AlignmentParams::prokaryotic(),
        "repetitive" => AlignmentParams::repetitive(),
        _ => return Err(anyhow::anyhow!("Unknown preset: {}. Available presets: bacterial, plant_te, mammal, viral, fungal, metagenomic, plant_genome, prokaryotic, repetitive", preset)),
    };
    
    // Initialize aligner
    let aligner = Minimap2Aligner::new(None)?;
    log::info!("Using aligner: {} {}", aligner.name(), aligner.version());
    
    // Run alignment
    let result = aligner.align(&reference, &query, &params, &output)?;
    
    log::info!("Alignment completed in {:.2}s", result.runtime_seconds);
    log::info!("Output: {} ({} records)", output.display(), result.stats.total_records);
    log::info!("Mean identity: {:.3}", result.stats.mean_identity);
    
    Ok(())
}

fn cmd_tile(paf: PathBuf, project: PathBuf, lod_spec: String, use_gpu: bool) -> Result<()> {
    log::info!("Building tiles from {}", paf.display());
    
    // Parse LOD specification
    let lod_levels = parse_lod_spec(&lod_spec)?;
    log::info!("Building LOD levels: {:?}", lod_levels);
    
    // Read PAF file
    let mut paf_reader = PafReader::new(&paf)?;
    let mut records = Vec::new();
    
    for record_result in paf_reader.records() {
        records.push(record_result?);
    }
    
    log::info!("Loaded {} PAF records", records.len());
    
    // Create dummy genome info - in practice would be loaded from FASTA indices
    let mut ref_genome = GenomeInfo::new();
    ref_genome.add_contig("ref".to_string(), 1_000_000_000); // 1Gb dummy
    
    let mut qry_genome = GenomeInfo::new();  
    qry_genome.add_contig("qry".to_string(), 1_000_000_000); // 1Gb dummy
    
    // Build tiles for each LOD level
    for lod in lod_levels {
        log::info!("Building LOD {}", lod);
        
        let grid = GridParams::new(lod, ref_genome.clone(), qry_genome.clone());
        
        let tiles = if use_gpu {
            // Try GPU pipeline first
            log::info!("Attempting GPU tile building...");
            match pollster::block_on(GpuPipeline::new()) {
                Ok(Some(gpu_pipeline)) => {
                    log::info!("Using GPU: {}", gpu_pipeline.get_device_info());
                    gpu_pipeline.build_tiles(&records, &grid)?
                }
                Ok(None) => {
                    log::warn!("GPU not available, falling back to CPU");
                    let cpu_pipeline = CpuPipeline;
                    cpu_pipeline.build_tiles(&records, &grid)?
                }
                Err(e) => {
                    log::warn!("GPU initialization failed: {}, falling back to CPU", e);
                    let cpu_pipeline = CpuPipeline;
                    cpu_pipeline.build_tiles(&records, &grid)?
                }
            }
        } else {
            log::info!("Using CPU tile building");
            let cpu_pipeline = CpuPipeline;
            cpu_pipeline.build_tiles(&records, &grid)?
        };
        
        log::info!("Built {} tiles for LOD {}", tiles.len(), lod);
        
        // TODO: Store tiles in project file
    }
    
    log::info!("Tile building completed");
    Ok(())
}

fn cmd_plot(_project: PathBuf, _output: PathBuf, _view: Option<String>, _width: u32, _height: u32) -> Result<()> {
    log::info!("Plot generation not yet implemented");
    Ok(())
}

fn cmd_quick(reference: PathBuf, query: PathBuf, preset: String, export: PathBuf, use_gpu: bool) -> Result<()> {
    log::info!("Quick comparison: {} vs {}", reference.display(), query.display());
    
    // Create temporary files
    let temp_dir = std::env::temp_dir();
    let paf_file = temp_dir.join("dotx_quick.paf");
    let project_file = temp_dir.join("dotx_quick.dotx");
    
    // Step 1: Align
    cmd_align(reference, query, preset, paf_file.clone(), 4)?;
    
    // Step 2: Tile  
    cmd_tile(paf_file, project_file.clone(), "0..5".to_string(), use_gpu)?;
    
    // Step 3: Plot
    cmd_plot(project_file, export, None, 1200, 800)?;
    
    log::info!("Quick comparison completed");
    Ok(())
}

fn cmd_project(action: ProjectCommands) -> Result<()> {
    match action {
        ProjectCommands::New { name, output } => {
            let project_file = output.join(format!("{}.dotx", name));
            let project = DotXProject::new(name.clone(), "Created via CLI".to_string());
            project.save(&project_file)?;
            log::info!("Created new project: {}", project_file.display());
        }
        ProjectCommands::Info { project } => {
            let project = DotXProject::load(&project)?;
            println!("Project: {}", project.manifest.project.name);
            println!("Description: {}", project.manifest.project.description);
            println!("Created: {}", project.manifest.created);
            println!("Inputs: {}", project.manifest.inputs.len());
            println!("Alignments: {}", project.manifest.alignments.len());
        }
        ProjectCommands::ListTiles { project, lod } => {
            let _project = DotXProject::load(&project)?;
            // TODO: List tiles from project storage
            log::info!("Listing tiles for LOD {:?}", lod);
        }
    }
    
    Ok(())
}

fn parse_lod_spec(spec: &str) -> Result<Vec<u16>> {
    let mut levels = Vec::new();
    
    if spec.contains("..") {
        // Range specification like "0..10"
        let parts: Vec<&str> = spec.split("..").collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid LOD range: {}", spec));
        }
        
        let start: u16 = parts[0].parse()?;
        let end: u16 = parts[1].parse()?;
        
        for level in start..=end {
            levels.push(level);
        }
    } else {
        // Comma-separated list like "5,6,7"
        for part in spec.split(',') {
            levels.push(part.trim().parse()?);
        }
    }
    
    Ok(levels)
}