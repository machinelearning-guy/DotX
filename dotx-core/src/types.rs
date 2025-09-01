use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use bytemuck::{Pod, Zeroable};

pub type GenomicPos = u64;
pub type TileIndex = u64;
pub type LodLevel = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenomicCoord {
    pub contig_id: u32,
    pub position: GenomicPos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenomicInterval {
    pub contig_id: u32,
    pub start: GenomicPos,
    pub end: GenomicPos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileCoord {
    pub lod: LodLevel,
    pub tile_x: TileIndex,
    pub tile_y: TileIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Pod, Zeroable)]
#[repr(C)]
pub struct BinData {
    pub count: u32,
    pub sum_len: u32,
    pub sum_identity: u32,
    pub strand_balance: i32,
}

impl Default for BinData {
    fn default() -> Self {
        Self {
            count: 0,
            sum_len: 0,
            sum_identity: 0,
            strand_balance: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContigInfo {
    pub id: u32,
    pub name: String,
    pub length: GenomicPos,
    pub offset: GenomicPos, // Global coordinate offset
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenomeInfo {
    pub contigs: Vec<ContigInfo>,
    pub total_length: GenomicPos,
    pub contig_map: HashMap<String, u32>,
}

impl GenomeInfo {
    pub fn new() -> Self {
        Self {
            contigs: Vec::new(),
            total_length: 0,
            contig_map: HashMap::new(),
        }
    }

    pub fn add_contig(&mut self, name: String, length: GenomicPos) -> u32 {
        let id = self.contigs.len() as u32;
        let offset = self.total_length;
        
        self.contigs.push(ContigInfo {
            id,
            name: name.clone(),
            length,
            offset,
        });
        
        self.contig_map.insert(name, id);
        self.total_length += length;
        id
    }

    pub fn get_contig(&self, id: u32) -> Option<&ContigInfo> {
        self.contigs.get(id as usize)
    }

    pub fn get_contig_by_name(&self, name: &str) -> Option<&ContigInfo> {
        self.contig_map.get(name).and_then(|&id| self.get_contig(id))
    }

    pub fn global_to_local(&self, global_pos: GenomicPos) -> Option<GenomicCoord> {
        // Validate global position is within total genome length
        if global_pos >= self.total_length {
            return None;
        }
        
        for contig in &self.contigs {
            let contig_end = contig.offset.saturating_add(contig.length);
            if global_pos >= contig.offset && global_pos < contig_end {
                let local_pos = global_pos.saturating_sub(contig.offset);
                // Validate local position doesn't exceed contig length
                if local_pos < contig.length {
                    return Some(GenomicCoord {
                        contig_id: contig.id,
                        position: local_pos,
                    });
                }
            }
        }
        None
    }

    pub fn local_to_global(&self, coord: GenomicCoord) -> Option<GenomicPos> {
        self.get_contig(coord.contig_id).and_then(|contig| {
            // Validate local position is within contig bounds
            if coord.position < contig.length {
                // Use saturating addition to prevent overflow
                Some(contig.offset.saturating_add(coord.position))
            } else {
                None
            }
        })
    }

    /// Validate that a genomic interval is valid within this genome
    pub fn validate_interval(&self, interval: &GenomicInterval) -> bool {
        if let Some(contig) = self.get_contig(interval.contig_id) {
            interval.start < interval.end && 
            interval.start < contig.length && 
            interval.end <= contig.length
        } else {
            false
        }
    }

    /// Clamp a genomic position to valid bounds within the genome
    pub fn clamp_position(&self, pos: GenomicPos) -> GenomicPos {
        pos.min(self.total_length.saturating_sub(1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Strand {
    Forward,
    Reverse,
}

impl From<bool> for Strand {
    fn from(forward: bool) -> Self {
        if forward {
            Strand::Forward
        } else {
            Strand::Reverse
        }
    }
}

impl From<Strand> for bool {
    fn from(strand: Strand) -> Self {
        matches!(strand, Strand::Forward)
    }
}

impl From<char> for Strand {
    fn from(c: char) -> Self {
        match c {
            '+' => Strand::Forward,
            '-' => Strand::Reverse,
            _ => Strand::Forward, // Default to forward
        }
    }
}

impl From<Strand> for char {
    fn from(strand: Strand) -> Self {
        match strand {
            Strand::Forward => '+',
            Strand::Reverse => '-',
        }
    }
}