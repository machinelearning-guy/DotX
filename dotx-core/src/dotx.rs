use crate::types::*;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotXProject {
    pub manifest: ProjectManifest,
    pub ref_genome: GenomeInfo,
    pub qry_genome: GenomeInfo,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub app_version: String,
    pub created: String,
    pub project: ProjectInfo,
    pub inputs: Vec<InputFile>,
    pub alignments: Vec<AlignmentRun>,
    pub tiles: TileConfig,
    pub settings: ProjectSettings,
    pub gpu_tuning: GpuTuning,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFile {
    #[serde(rename = "type")]
    pub file_type: String,
    pub role: String, // "ref" or "qry"
    pub path: PathBuf,
    pub sha256: String,
    pub contigs: Vec<ContigInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentRun {
    pub id: String,
    pub source: String, // "minimap2", "preview", etc.
    pub file: PathBuf,
    pub params: HashMap<String, serde_json::Value>,
    pub stats: AlignmentStats,
    pub lod_built: Vec<LodLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentStats {
    pub records: u64,
    pub identity_mean: f64,
    pub identity_median: f64,
    pub strand_balance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileConfig {
    pub format: String,
    pub tile_size: u32,
    pub lod_max: LodLevel,
    pub index: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub color_map: String,
    pub min_identity: f64,
    pub max_identity: f64,
    pub min_length: GenomicPos,
    pub viewport: Option<ViewportState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportState {
    pub ref_start: GenomicPos,
    pub ref_end: GenomicPos,
    pub qry_start: GenomicPos,
    pub qry_end: GenomicPos,
    pub zoom_level: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuTuning {
    pub workgroup: u32,
    pub deterministic_merge: bool,
    pub cache_size_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub platform: String,
    pub rustc: String,
    pub wgpu: String,
    pub device_name: Option<String>,
    pub compute_units: Option<u32>,
}

impl DotXProject {
    pub fn new(name: String, description: String) -> Self {
        let manifest = ProjectManifest {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            created: "2025-01-01T00:00:00Z".to_string(), // TODO: Use chrono when available
            project: ProjectInfo { name, description },
            inputs: Vec::new(),
            alignments: Vec::new(),
            tiles: TileConfig {
                format: "v1".to_string(),
                tile_size: 512,
                lod_max: 38,
                index: PathBuf::from("tiles/index.lmdb"),
            },
            settings: ProjectSettings {
                color_map: "density".to_string(),
                min_identity: 0.0,
                max_identity: 1.0,
                min_length: 0,
                viewport: None,
            },
            gpu_tuning: GpuTuning {
                workgroup: 256,
                deterministic_merge: true,
                cache_size_mb: 512,
            },
            provenance: Provenance {
                platform: std::env::consts::OS.to_string(),
                rustc: "1.80.0".to_string(), // TODO: Get from build
                wgpu: "22.0".to_string(),
                device_name: None,
                compute_units: None,
            },
        };

        Self {
            manifest,
            ref_genome: GenomeInfo::new(),
            qry_genome: GenomeInfo::new(),
            path: PathBuf::new(),
        }
    }

    pub fn save(&self, path: &PathBuf) -> Result<()> {
        use std::fs::File;
        use std::io::Write;
        use zip::write::{FileOptions, ZipWriter};

        let file = File::create(path)?;
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::<zip::write::ExtendedFileOptions>::default()
            .compression_method(zip::CompressionMethod::Stored); // Use stored without compression level

        // Write manifest
        zip.start_file("manifest.json", options)?;
        let manifest_json = serde_json::to_string_pretty(&self.manifest)?;
        zip.write_all(manifest_json.as_bytes())?;

        // TODO: Write other project components
        // - inputs/ directory with embedded FASTA files (optional)
        // - alignments/ directory with PAF files
        // - tiles/ directory with tile data
        // - annotations/ directory with GFF3/GenBank data

        zip.finish()?;
        Ok(())
    }

    pub fn load(path: &PathBuf) -> Result<Self> {
        use std::fs::File;
        use std::io::Read;
        use zip::read::ZipArchive;

        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file)?;

        // Read manifest
        let mut manifest_file = archive.by_name("manifest.json")?;
        let mut manifest_content = String::new();
        manifest_file.read_to_string(&mut manifest_content)?;
        let manifest: ProjectManifest = serde_json::from_str(&manifest_content)?;

        // TODO: Load other project components
        // For now, create empty genomes - they'll be populated when loading inputs
        let ref_genome = GenomeInfo::new();
        let qry_genome = GenomeInfo::new();

        Ok(Self {
            manifest,
            ref_genome,
            qry_genome,
            path: path.clone(),
        })
    }

    pub fn add_input(&mut self, file_type: String, role: String, path: PathBuf, genome_info: GenomeInfo) -> Result<()> {
        // Calculate SHA256 of the file
        let sha256 = "todo_calculate_hash".to_string(); // TODO: Implement proper hashing

        let input_file = InputFile {
            file_type,
            role: role.clone(),
            path,
            sha256,
            contigs: genome_info.contigs.clone(),
        };

        self.manifest.inputs.push(input_file);

        // Update the appropriate genome
        match role.as_str() {
            "ref" => self.ref_genome = genome_info,
            "qry" => self.qry_genome = genome_info,
            _ => return Err(anyhow!("Invalid role: {}", role)),
        }

        Ok(())
    }

    pub fn add_alignment(&mut self, id: String, source: String, file: PathBuf, params: HashMap<String, serde_json::Value>) -> Result<()> {
        // TODO: Calculate stats from PAF file
        let stats = AlignmentStats {
            records: 0,
            identity_mean: 0.0,
            identity_median: 0.0,
            strand_balance: 0.5,
        };

        let alignment_run = AlignmentRun {
            id,
            source,
            file,
            params,
            stats,
            lod_built: Vec::new(),
        };

        self.manifest.alignments.push(alignment_run);
        Ok(())
    }

    pub fn get_alignment(&self, id: &str) -> Option<&AlignmentRun> {
        self.manifest.alignments.iter().find(|a| a.id == id)
    }

    pub fn get_alignment_mut(&mut self, id: &str) -> Option<&mut AlignmentRun> {
        self.manifest.alignments.iter_mut().find(|a| a.id == id)
    }
}