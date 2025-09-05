//! Serialization and deserialization utilities for DOTx data structures

use serde::{Deserialize, Serialize};
use anyhow::{Result, anyhow};
use std::io::{Read, Write};
use std::path::Path;
use crate::types::{Anchor, Chain};
use crate::dotxdb::{DotxdbFile, Sample, ContigInfo, DeltaAnchor};
use std::collections::HashMap;

/// Serialization format options
#[derive(Debug, Clone, Copy)]
pub enum SerializationFormat {
    /// Binary format with bincode
    Binary,
    /// JSON format (less efficient but human readable)
    Json,
    /// Compressed binary format with zstd
    CompressedBinary,
}

/// Serialization configuration
#[derive(Debug, Clone)]
pub struct SerializationConfig {
    pub format: SerializationFormat,
    pub compression_level: i32,
    pub pretty_json: bool,
}

impl Default for SerializationConfig {
    fn default() -> Self {
        Self {
            format: SerializationFormat::CompressedBinary,
            compression_level: 6,
            pretty_json: false,
        }
    }
}

/// High-level serialization interface
pub struct Serializer {
    config: SerializationConfig,
}

impl Serializer {
    pub fn new(config: SerializationConfig) -> Self {
        Self { config }
    }

    /// Serialize data to bytes
    pub fn serialize<T: Serialize>(&self, data: &T) -> Result<Vec<u8>> {
        match self.config.format {
            SerializationFormat::Binary => {
                Ok(bincode::serialize(data)?)
            },
            SerializationFormat::Json => {
                let json = if self.config.pretty_json {
                    serde_json::to_vec_pretty(data)?
                } else {
                    serde_json::to_vec(data)?
                };
                Ok(json)
            },
            SerializationFormat::CompressedBinary => {
                let serialized = bincode::serialize(data)?;
                let mut encoder = zstd::Encoder::new(Vec::new(), self.config.compression_level)?;
                encoder.write_all(&serialized)?;
                Ok(encoder.finish()?)
            },
        }
    }

    /// Deserialize data from bytes
    pub fn deserialize<T: for<'de> Deserialize<'de>>(&self, data: &[u8]) -> Result<T> {
        match self.config.format {
            SerializationFormat::Binary => {
                Ok(bincode::deserialize(data)?)
            },
            SerializationFormat::Json => {
                Ok(serde_json::from_slice(data)?)
            },
            SerializationFormat::CompressedBinary => {
                let decompressed = zstd::decode_all(data)?;
                Ok(bincode::deserialize(&decompressed)?)
            },
        }
    }

    /// Serialize to file
    pub fn serialize_to_file<T: Serialize, P: AsRef<Path>>(&self, data: &T, path: P) -> Result<()> {
        let bytes = self.serialize(data)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Deserialize from file
    pub fn deserialize_from_file<T: for<'de> Deserialize<'de>, P: AsRef<Path>>(&self, path: P) -> Result<T> {
        let bytes = std::fs::read(path)?;
        self.deserialize(&bytes)
    }
}

/// Builder for creating DotxdbFile from collections of anchors and chains
pub struct DotxdbBuilder {
    file: DotxdbFile,
    contig_map: HashMap<String, u32>,
    engine_map: HashMap<String, u8>,
    engine_names: Vec<String>,
}

impl DotxdbBuilder {
    pub fn new(compression_level: i32) -> Self {
        Self {
            file: DotxdbFile::new(compression_level),
            contig_map: HashMap::new(),
            engine_map: HashMap::new(),
            engine_names: Vec::new(),
        }
    }

    /// Add a sample to the database
    pub fn add_sample(&mut self, sample: Sample) -> &mut Self {
        self.file.add_sample(sample);
        self
    }

    /// Add a contig to the database
    pub fn add_contig(&mut self, contig: ContigInfo) -> &mut Self {
        let index = self.contig_map.len() as u32;
        self.contig_map.insert(contig.name.clone(), index);
        self.file.add_contig(contig);
        self
    }

    /// Register an engine name and get its index
    pub fn register_engine(&mut self, engine_name: String) -> u8 {
        if let Some(&idx) = self.engine_map.get(&engine_name) {
            return idx;
        }
        
        let idx = self.engine_names.len() as u8;
        self.engine_map.insert(engine_name.clone(), idx);
        self.engine_names.push(engine_name);
        idx
    }

    /// Add anchors to the database
    pub fn add_anchors(&mut self, anchors: Vec<Anchor>) -> Result<&mut Self> {
        let mut prev_qs = 0;
        let mut prev_ts = 0;

        // Ensure all engines are registered
        for anchor in &anchors {
            self.register_engine(anchor.engine_tag.clone());
        }

        // Convert to delta-encoded format
        for anchor in anchors {
            let delta = DeltaAnchor::encode(
                &anchor, 
                &self.contig_map, 
                &self.engine_map, 
                &mut prev_qs, 
                &mut prev_ts
            )?;
            self.file.anchors.push(delta);
        }

        Ok(self)
    }

    /// Add chains to the database
    pub fn add_chains(&mut self, chains: Vec<Chain>) -> Result<&mut Self> {
        let mut anchor_offset = 0;

        for chain in chains {
            let anchor_count = chain.anchors.len() as u32;
            
            // Add the anchors first
            self.add_anchors(chain.anchors)?;

            // Add the chain index
            self.file.chains.push(crate::dotxdb::ChainIndex {
                chain_id: chain.chain_id,
                anchor_start: anchor_offset,
                anchor_count,
                score: chain.score,
            });

            anchor_offset += anchor_count as u64;
        }

        Ok(self)
    }

    /// Build the final DotxdbFile
    pub fn build(mut self) -> DotxdbFile {
        self.file.update_stats();
        
        // Calculate file offsets (simplified - in real implementation would be more precise)
        let header_size = 1024; // Approximate header size
        let meta_size = bincode::serialized_size(&self.file.meta).unwrap_or(4096);
        
        self.file.meta.index_offsets.header_offset = 0;
        self.file.meta.index_offsets.meta_offset = header_size;
        self.file.meta.index_offsets.anchors_offset = header_size + meta_size;
        
        // More precise calculation would be done here for other offsets
        
        self.file
    }
}

/// Utility functions for working with DOTx data
pub mod utils {
    use super::*;
    use crate::types::{Anchor, Chain, Strand};

    /// Convert PAF-like text format to anchors
    pub fn parse_paf_to_anchors(paf_content: &str, engine_tag: String) -> Result<Vec<Anchor>> {
        let mut anchors = Vec::new();
        
        for line in paf_content.lines() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 12 {
                continue;
            }
            
            let q_name = fields[0].to_string();
            let q_len: u64 = fields[1].parse()?;
            let q_start: u64 = fields[2].parse()?;
            let q_end: u64 = fields[3].parse()?;
            let strand = if fields[4] == "+" { Strand::Forward } else { Strand::Reverse };
            let t_name = fields[5].to_string();
            let t_len: u64 = fields[6].parse()?;
            let t_start: u64 = fields[7].parse()?;
            let t_end: u64 = fields[8].parse()?;
            let mapq: Option<u8> = fields[11].parse().ok();
            
            let mut anchor = Anchor::new(
                q_name,
                t_name,
                q_start,
                q_end,
                t_start,
                t_end,
                strand,
                engine_tag.clone(),
            );
            
            if let Some(mq) = mapq {
                anchor = anchor.with_mapq(mq);
            }
            
            // Parse optional identity if present
            if fields.len() > 12 {
                if let Ok(identity) = fields[12].parse::<f32>() {
                    anchor = anchor.with_identity(identity);
                }
            }
            
            anchors.push(anchor);
        }
        
        Ok(anchors)
    }

    /// Group anchors into chains based on collinearity
    pub fn chain_anchors(mut anchors: Vec<Anchor>, max_gap: u64) -> Vec<Chain> {
        if anchors.is_empty() {
            return Vec::new();
        }

        // Sort anchors by query and target positions
        anchors.sort_by_key(|a| (a.q.clone(), a.qs, a.ts));

        let mut chains = Vec::new();
        let mut current_chain = Chain::new(0);
        let mut chain_id = 0;

        for anchor in anchors {
            let should_start_new_chain = if current_chain.is_empty() {
                false
            } else {
                let last_anchor = current_chain.anchors.last().unwrap();
                // Start new chain if different contigs or large gap
                anchor.q != last_anchor.q 
                    || anchor.t != last_anchor.t
                    || anchor.qs.saturating_sub(last_anchor.qe) > max_gap
                    || anchor.ts.saturating_sub(last_anchor.te) > max_gap
            };

            if should_start_new_chain && !current_chain.is_empty() {
                current_chain.chain_id = chain_id;
                chains.push(current_chain);
                current_chain = Chain::new(chain_id + 1);
                chain_id += 1;
            }

            current_chain.add_anchor(anchor);
        }

        if !current_chain.is_empty() {
            current_chain.chain_id = chain_id;
            chains.push(current_chain);
        }

        chains
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Anchor, Chain, Strand};

    #[test]
    fn test_binary_serialization() {
        let config = SerializationConfig {
            format: SerializationFormat::Binary,
            ..Default::default()
        };
        let serializer = Serializer::new(config);

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

        let serialized = serializer.serialize(&anchor).unwrap();
        let deserialized: Anchor = serializer.deserialize(&serialized).unwrap();

        assert_eq!(anchor.q, deserialized.q);
        assert_eq!(anchor.qs, deserialized.qs);
    }

    #[test]
    fn test_compressed_serialization() {
        let config = SerializationConfig {
            format: SerializationFormat::CompressedBinary,
            compression_level: 3,
            ..Default::default()
        };
        let serializer = Serializer::new(config);

        let anchors: Vec<Anchor> = (0..100).map(|i| {
            Anchor::new(
                format!("chr{}", i % 5),
                format!("chr{}", (i + 1) % 5),
                i * 1000,
                i * 1000 + 500,
                i * 2000,
                i * 2000 + 600,
                if i % 2 == 0 { Strand::Forward } else { Strand::Reverse },
                "test_engine".to_string(),
            )
        }).collect();

        let serialized = serializer.serialize(&anchors).unwrap();
        let deserialized: Vec<Anchor> = serializer.deserialize(&serialized).unwrap();

        assert_eq!(anchors.len(), deserialized.len());
        assert_eq!(anchors[0].q, deserialized[0].q);
    }

    #[test]
    fn test_dotxdb_builder() {
        let mut builder = DotxdbBuilder::new(6);
        
        // Add sample and contigs
        builder.add_sample(Sample {
            name: "test_sample".to_string(),
            path: "/path/to/sample.fa".to_string(),
            description: Some("Test sample".to_string()),
            total_length: 10000,
            num_contigs: 2,
            checksum: None,
        });
        
        builder.add_contig(ContigInfo {
            name: "chr1".to_string(),
            length: 5000,
            sample_id: "test_sample".to_string(),
            index: 0,
        });
        
        builder.add_contig(ContigInfo {
            name: "chr2".to_string(),
            length: 5000,
            sample_id: "test_sample".to_string(),
            index: 1,
        });

        // Add some test anchors
        let anchors = vec![
            Anchor::new(
                "chr1".to_string(),
                "chr2".to_string(),
                100,
                200,
                300,
                400,
                Strand::Forward,
                "minimap2".to_string(),
            ),
        ];

        builder.add_anchors(anchors).unwrap();
        let file = builder.build();

        assert_eq!(file.meta.samples.len(), 1);
        assert_eq!(file.meta.contigs.len(), 2);
        assert_eq!(file.anchors.len(), 1);
    }

    #[test]
    fn test_paf_parsing() {
        let paf_content = "chr1\t5000\t100\t200\t+\tchr2\t5000\t300\t400\t100\t100\t60\t0.95";
        let anchors = utils::parse_paf_to_anchors(paf_content, "minimap2".to_string()).unwrap();
        
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].q, "chr1");
        assert_eq!(anchors[0].t, "chr2");
        assert_eq!(anchors[0].qs, 100);
        assert_eq!(anchors[0].qe, 200);
        assert_eq!(anchors[0].ts, 300);
        assert_eq!(anchors[0].te, 400);
        assert_eq!(anchors[0].strand, Strand::Forward);
    }

    #[test]
    fn test_chaining() {
        let anchors = vec![
            Anchor::new("chr1".to_string(), "chr2".to_string(), 100, 200, 300, 400, Strand::Forward, "test".to_string()),
            Anchor::new("chr1".to_string(), "chr2".to_string(), 250, 350, 450, 550, Strand::Forward, "test".to_string()),
            Anchor::new("chr1".to_string(), "chr3".to_string(), 400, 500, 100, 200, Strand::Forward, "test".to_string()),
        ];

        let chains = utils::chain_anchors(anchors, 100);
        assert_eq!(chains.len(), 2); // Two chains: chr1->chr2 and chr1->chr3
        assert_eq!(chains[0].len(), 2); // First chain has 2 anchors
        assert_eq!(chains[1].len(), 1); // Second chain has 1 anchor
    }
}