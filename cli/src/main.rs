use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use anyhow::{Result, Context};

mod config;
mod commands;

use config::Config;

#[derive(Parser)]
#[command(name = "dotx")]
#[command(about = "DOTx - Supercharged Dot-Plot Engine")]
#[command(version)]
#[command(long_about = "
DOTx is a fast, precise, general-purpose dot plot engine for genome analysis.
It scales from plasmids to whole genomes with interactive plots and various alignment engines.

Examples:
  dotx map --ref genome.fa --qry reads.fa --out alignment.paf
  dotx import --paf alignment.paf --db data.dotxdb --build-tiles  
  dotx render --db data.dotxdb --out plot.svg --dpi 300
  dotx refine --db data.dotxdb --roi 'chr1:1M-2M' --engine wfa
  dotx gui --db data.dotxdb
")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    /// Configuration file path
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    
    /// Enable deterministic mode for reproducible results
    #[arg(long, global = true)]
    pub deterministic: bool,
    
    /// Number of threads to use
    #[arg(short, long, global = true)]
    pub threads: Option<usize>,
    
    /// Verbose output
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    
    /// Quiet mode (suppress non-error output)
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Align sequences with various engines (minimap2, syncmer, strobemer)
    #[cfg(feature = "map")]
    Map {
        /// Reference sequence file (FASTA/FASTQ)
        #[arg(long, required = true)]
        r#ref: PathBuf,
        
        /// Query sequence file (FASTA/FASTQ)
        #[arg(long, required = true)]
        qry: PathBuf,
        
        /// Output file (PAF format)
        #[arg(short, long, required = true)]
        out: PathBuf,
        
        /// Alignment engine to use
        #[arg(long, default_value = "minimap2")]
        engine: EngineType,
        
        /// Minimap2 preset (when using minimap2 engine)
        #[arg(long, default_value = "asm5")]
        preset: Option<String>,
        
        /// K-mer size for seeding
        #[arg(short, long)]
        k: Option<u32>,
        
        /// Syncmer size parameter (s)
        #[arg(long)]
        syncmer_s: Option<u32>,
        
        /// Syncmer threshold parameter (t)
        #[arg(long)]
        syncmer_t: Option<u32>,
        
        /// Strobemer window size
        #[arg(long)]
        strobemer_window: Option<u32>,
        
        /// Maximum frequency threshold for seed filtering
        #[arg(long)]
        max_freq: Option<u32>,
        
        /// Minimum anchor length
        #[arg(long)]
        min_anchor_len: Option<u32>,
        
        /// Enable low-complexity masking
        #[arg(long)]
        mask_low_complexity: bool,
        
        /// Additional engine arguments
        #[arg(long)]
        extra_args: Vec<String>,
    },
    
    /// Convert PAF/MAF/MUMmer/SAM to .dotxdb format
    #[cfg(feature = "import")]
    Import {
        /// Input file path
        #[arg(short, long)]
        input: PathBuf,
        
        /// Input file format (auto-detected if not specified)
        #[arg(long)]
        format: Option<FormatType>,
        
        /// Output database file (.dotxdb)
        #[arg(long, required = true)]
        db: PathBuf,
        
        /// Build tile index for fast rendering
        #[arg(long)]
        build_tiles: bool,
        
        /// Reference sequence file (for coordinate validation)
        #[arg(long)]
        r#ref: Option<PathBuf>,
        
        /// Query sequence file (for coordinate validation)
        #[arg(long)]
        qry: Option<PathBuf>,
        
        /// Compression level (0-9)
        #[arg(long, default_value = "6")]
        compression: u8,
    },
    
    /// Export plots to SVG/PNG/PDF with configuration options
    #[cfg(feature = "render")]
    Render {
        /// Input database file (.dotxdb)
        #[arg(long, required = true)]
        db: PathBuf,
        
        /// Output file (SVG/PNG/PDF)
        #[arg(short, long, required = true)]
        out: PathBuf,
        
        /// Output format (auto-detected from extension)
        #[arg(long)]
        format: Option<RenderFormat>,
        
        /// Strands to show ('+', '-', or '+,-')
        #[arg(long, default_value = "+,-")]
        strand: String,
        
        /// Axis flip/swap operations
        #[arg(long, default_value = "none")]
        flip: FlipMode,
        
        /// Color theme
        #[arg(long, default_value = "default")]
        theme: String,
        
        /// DPI for raster outputs
        #[arg(long, default_value = "300")]
        dpi: u32,
        
        /// Width in pixels
        #[arg(long)]
        width: Option<u32>,
        
        /// Height in pixels  
        #[arg(long)]
        height: Option<u32>,
        
        /// Region to render (e.g., 'chr1:1M-2M,chr2:500K-1.5M')
        #[arg(long)]
        region: Option<String>,
        
        /// Include legend
        #[arg(long, default_value = "true")]
        legend: bool,
        
        /// Include scale bar
        #[arg(long, default_value = "true")]
        scale_bar: bool,
    },
    
    /// Compute exact alignments on ROI tiles
    #[cfg(feature = "refine")]
    Refine {
        /// Database file to refine (.dotxdb)
        #[arg(long, required = true)]
        db: PathBuf,
        
        /// Regions of interest (e.g., 'chr1:12.3M-18.6M,chr2:21.1M-27.2M')
        #[arg(long)]
        roi: Option<String>,
        
        /// Exact alignment engine
        #[arg(long, default_value = "wfa")]
        engine: RefineEngine,
        
        /// Compute device (cpu or gpu)
        #[arg(long, default_value = "cpu")]
        device: DeviceType,
        
        /// Batch size for GPU processing
        #[arg(long)]
        batch_size: Option<u32>,
        
        /// Maximum alignment length for exact computation
        #[arg(long)]
        max_align_len: Option<u32>,
        
        /// Reference sequence file
        #[arg(long)]
        r#ref: Option<PathBuf>,
        
        /// Query sequence file
        #[arg(long)]
        qry: Option<PathBuf>,
    },
    
    /// Launch the desktop GUI (Tauri). Optional: open a DB on start.
    #[cfg(feature = "gui")]
    Gui {
        /// Database file to open on launch (.dotxdb)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum EngineType {
    Minimap2,
    Syncmer,
    Strobemer,
    Kmer,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum FormatType {
    Paf,
    Maf,
    Mummer,
    Sam,
    Bam,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum RenderFormat {
    Svg,
    Png,
    Pdf,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum FlipMode {
    None,
    X,
    Y,
    Xy,
    Rcx,
    Rcy,
    Rcxy,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum RefineEngine {
    Wfa,
    Edlib,
    Custom,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum DeviceType {
    Cpu,
    Gpu,
}

fn setup_logging(verbose: u8, quiet: bool) -> Result<()> {
    if quiet {
        std::env::set_var("RUST_LOG", "error");
    } else {
        let level = match verbose {
            0 => "info",
            1 => "debug", 
            _ => "trace",
        };
        std::env::set_var("RUST_LOG", level);
    }
    
    env_logger::Builder::from_default_env()
        .format_timestamp_secs()
        .init();
    
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Setup logging
    setup_logging(cli.verbose, cli.quiet)?;
    
    // Load configuration
    let config = Config::load(cli.config.as_ref().map(|v| v.as_path()))?;
    
    // Set global thread count if specified
    if let Some(threads) = cli.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .context("Failed to set thread count")?;
    }
    
    // Execute the requested command
    match cli.command {
        #[cfg(feature = "map")]
        Commands::Map { 
            r#ref,
            qry,
            out,
            engine,
            preset,
            k,
            syncmer_s,
            syncmer_t,
            strobemer_window,
            max_freq,
            min_anchor_len,
            mask_low_complexity,
            extra_args,
        } => {
            commands::map::execute(
                &config, 
                cli.deterministic,
                r#ref,
                qry,
                out,
                engine,
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
        }
        
        #[cfg(feature = "import")]
        Commands::Import { 
            input,
            format,
            db,
            build_tiles,
            r#ref,
            qry,
            compression,
        } => {
            commands::import::execute(
                &config,
                input,
                format,
                db,
                build_tiles,
                r#ref,
                qry,
                compression,
            )?;
        }
        
        #[cfg(feature = "render")]
        Commands::Render {
            db,
            out,
            format,
            strand,
            flip,
            theme,
            dpi,
            width,
            height,
            region,
            legend,
            scale_bar,
        } => {
            commands::render::execute(
                &config,
                db,
                out,
                format,
                strand,
                flip,
                theme,
                dpi,
                width,
                height,
                region,
                legend,
                scale_bar,
            )?;
        }
        
        #[cfg(feature = "refine")]
        Commands::Refine {
            db,
            roi,
            engine,
            device,
            batch_size,
            max_align_len,
            r#ref,
            qry,
        } => {
            commands::refine::execute(
                &config,
                db,
                roi,
                engine,
                device,
                batch_size,
                max_align_len,
                r#ref,
                qry,
            )?;
        }
        
        #[cfg(feature = "gui")]
        Commands::Gui { db } => {
            commands::gui::execute(db)?;
        }
    }
    
    Ok(())
}
