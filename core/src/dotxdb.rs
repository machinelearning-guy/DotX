use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use bincode;
use zstd;
use std::io::{Read, Write};

/// Magic bytes that identify a .dotxdb file
pub const DOTXDB_MAGIC: &[u8; 4] = b"DOTX";

/// Current version of the .dotxdb format
pub const DOTXDB_VERSION: u32 = 1;

/// Build metadata embedded in the file header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildMetadata {
    pub version: String,
    pub build_date: String,
    pub git_commit: Option<String>,
    pub features: Vec<String>,
}

impl Default for BuildMetadata {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_date: chrono::Utc::now().to_rfc3339(),
            git_commit: option_env!("GIT_COMMIT").map(|s| s.to_string()),
            features: Vec::new(),
        }
    }
}

/// File header for .dotxdb format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotxdbHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub build_metadata: BuildMetadata,
    pub compression_level: i32,
    pub created_at: String,
}

impl DotxdbHeader {
    pub fn new(compression_level: i32) -> Self {
        Self {
            magic: *DOTXDB_MAGIC,
            version: DOTXDB_VERSION,
            build_metadata: BuildMetadata::default(),
            compression_level,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
    
    pub fn validate(&self) -> Result<()> {
        if self.magic != *DOTXDB_MAGIC {
            return Err(anyhow!("Invalid magic bytes: expected DOTX, got {:?}", self.magic));
        }
        if self.version > DOTXDB_VERSION {
            return Err(anyhow!("Unsupported version: {}, max supported: {}", self.version, DOTXDB_VERSION));
        }
        Ok(())
    }
}

/// Sample metadata for multi-genome analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub total_length: u64,
    pub num_contigs: u32,
    pub checksum: Option<String>,
}

/// Contig information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContigInfo {
    pub name: String,
    pub length: u64,
    pub sample_id: String,
    pub index: u32,
}

/// Meta section containing samples, contigs, and index offsets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaSection {
    pub samples: Vec<Sample>,
    pub contigs: Vec<ContigInfo>,
    pub total_anchors: u64,
    pub total_chains: u64,
    pub index_offsets: IndexOffsets,
}

/// Byte offsets for different sections in the file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexOffsets {
    pub header_offset: u64,
    pub meta_offset: u64,
    pub anchors_offset: u64,
    pub chains_offset: u64,
    pub tiles_offset: u64,
    pub verify_offset: u64,
    pub file_size: u64,
}

/// Delta-encoded anchor coordinates for efficient storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaAnchor {
    pub q_idx: u32,          // query contig index
    pub t_idx: u32,          // target contig index
    pub qs_delta: i64,       // delta from previous query start
    pub qe_delta: i32,       // delta from query start to end
    pub ts_delta: i64,       // delta from previous target start  
    pub te_delta: i32,       // delta from target start to end
    pub strand: u8,          // 0 = forward, 1 = reverse
    pub mapq: u8,            // mapping quality (0-255, 255 = missing)
    pub identity: u16,       // identity * 10000 (0-10000, 65535 = missing)
    pub engine_idx: u8,      // index into engine table
}

impl DeltaAnchor {
    pub fn encode(anchor: &crate::types::Anchor, 
                  contig_map: &HashMap<String, u32>,
                  engine_map: &HashMap<String, u8>,
                  prev_qs: &mut u64,
                  prev_ts: &mut u64) -> Result<Self> {
        let q_idx = contig_map.get(&anchor.q)
            .ok_or_else(|| anyhow!("Query contig not found: {}", anchor.q))?;
        let t_idx = contig_map.get(&anchor.t)
            .ok_or_else(|| anyhow!("Target contig not found: {}", anchor.t))?;
        let engine_idx = engine_map.get(&anchor.engine_tag)
            .ok_or_else(|| anyhow!("Engine not found: {}", anchor.engine_tag))?;
        
        let qs_delta = anchor.qs as i64 - *prev_qs as i64;
        let ts_delta = anchor.ts as i64 - *prev_ts as i64;
        
        *prev_qs = anchor.qs;
        *prev_ts = anchor.ts;
        
        Ok(DeltaAnchor {
            q_idx: *q_idx,
            t_idx: *t_idx,
            qs_delta,
            qe_delta: (anchor.qe - anchor.qs) as i32,
            ts_delta,
            te_delta: (anchor.te - anchor.ts) as i32,
            strand: match anchor.strand {
                crate::types::Strand::Forward => 0,
                crate::types::Strand::Reverse => 1,
            },
            mapq: anchor.mapq.unwrap_or(255),
            identity: anchor.identity.map(|i| (i * 10000.0) as u16).unwrap_or(65535),
            engine_idx: *engine_idx,
        })
    }
}

/// Chain index with range of anchors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainIndex {
    pub chain_id: u64,
    pub anchor_start: u64,   // index of first anchor in this chain
    pub anchor_count: u32,   // number of anchors in this chain
    pub score: f64,
}

/// Quadtree tile for spatial indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileIndex {
    pub level: u8,           // zoom level
    pub x: u32,              // tile x coordinate
    pub y: u32,              // tile y coordinate
    pub anchor_start: u64,   // index of first anchor in this tile
    pub anchor_count: u32,   // number of anchors in this tile
    pub data_offset: u64,    // byte offset to compressed tile data
    pub data_size: u32,      // size of compressed tile data
}

/// Region of Interest verification results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoiResult {
    pub query_contig: String,
    pub target_contig: String,
    pub query_start: u64,
    pub query_end: u64,
    pub target_start: u64,
    pub target_end: u64,
    pub anchor_count: u32,
    pub chain_count: u32,
    pub avg_identity: f32,
    pub max_score: f64,
}

/// Verification section for ROI results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySection {
    pub roi_results: Vec<RoiResult>,
    pub checksum: String,    // SHA256 of the entire file content
}

/// Complete .dotxdb file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotxdbFile {
    pub header: DotxdbHeader,
    pub meta: MetaSection,
    pub anchors: Vec<DeltaAnchor>,
    pub chains: Vec<ChainIndex>,
    pub tiles: Vec<TileIndex>,
    pub verify: VerifySection,
}

impl DotxdbFile {
    pub fn new(compression_level: i32) -> Self {
        Self {
            header: DotxdbHeader::new(compression_level),
            meta: MetaSection {
                samples: Vec::new(),
                contigs: Vec::new(),
                total_anchors: 0,
                total_chains: 0,
                index_offsets: IndexOffsets {
                    header_offset: 0,
                    meta_offset: 0,
                    anchors_offset: 0,
                    chains_offset: 0,
                    tiles_offset: 0,
                    verify_offset: 0,
                    file_size: 0,
                },
            },
            anchors: Vec::new(),
            chains: Vec::new(),
            tiles: Vec::new(),
            verify: VerifySection {
                roi_results: Vec::new(),
                checksum: String::new(),
            },
        }
    }

    /// Serialize and compress the file to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let serialized = bincode::serialize(self)?;
        let mut encoder = zstd::Encoder::new(Vec::new(), self.header.compression_level)?;
        encoder.write_all(&serialized)?;
        let compressed = encoder.finish()?;
        Ok(compressed)
    }

    /// Deserialize and decompress from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let decompressed = zstd::decode_all(data)?;
        let file: DotxdbFile = bincode::deserialize(&decompressed)?;
        file.header.validate()?;
        Ok(file)
    }

    /// Write to a file path
    pub fn write_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let data = self.to_bytes()?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Read from a file path
    pub fn read_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(&data)
    }

    /// Add a sample to the meta section
    pub fn add_sample(&mut self, sample: Sample) {
        self.meta.samples.push(sample);
    }

    /// Add a contig to the meta section
    pub fn add_contig(&mut self, contig: ContigInfo) {
        self.meta.contigs.push(contig);
    }

    /// Get contig name to index mapping
    pub fn get_contig_map(&self) -> HashMap<String, u32> {
        self.meta.contigs.iter()
            .enumerate()
            .map(|(i, c)| (c.name.clone(), i as u32))
            .collect()
    }

    /// Update statistics after adding data
    pub fn update_stats(&mut self) {
        self.meta.total_anchors = self.anchors.len() as u64;
        self.meta.total_chains = self.chains.len() as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Anchor, Strand};

    #[test]
    fn test_header_validation() {
        let header = DotxdbHeader::new(6);
        assert!(header.validate().is_ok());

        let mut invalid_header = header.clone();
        invalid_header.magic = *b"XXXX";
        assert!(invalid_header.validate().is_err());
    }

    #[test]
    fn test_delta_encoding() {
        let anchor = Anchor::new(
            "chr1".to_string(),
            "chr2".to_string(),
            1000,
            2000,
            3000,
            4000,
            Strand::Forward,
            "minimap2".to_string(),
        );

        let mut contig_map = HashMap::new();
        contig_map.insert("chr1".to_string(), 0);
        contig_map.insert("chr2".to_string(), 1);

        let mut engine_map = HashMap::new();
        engine_map.insert("minimap2".to_string(), 0);

        let mut prev_qs = 0;
        let mut prev_ts = 0;

        let delta = DeltaAnchor::encode(&anchor, &contig_map, &engine_map, &mut prev_qs, &mut prev_ts).unwrap();
        
        assert_eq!(delta.q_idx, 0);
        assert_eq!(delta.t_idx, 1);
        assert_eq!(delta.qs_delta, 1000);
        assert_eq!(delta.qe_delta, 1000);
        assert_eq!(delta.ts_delta, 3000);
        assert_eq!(delta.te_delta, 1000);
        assert_eq!(delta.strand, 0);
        assert_eq!(delta.engine_idx, 0);
    }

    #[test]
    fn test_file_serialization() {
        let file = DotxdbFile::new(6);
        let bytes = file.to_bytes().unwrap();
        let deserialized = DotxdbFile::from_bytes(&bytes).unwrap();
        
        assert_eq!(file.header.magic, deserialized.header.magic);
        assert_eq!(file.header.version, deserialized.header.version);
    }
}