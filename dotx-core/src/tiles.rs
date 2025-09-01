use crate::types::*;
use crate::coords::*;
use crate::paf::PafRecord;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Note: LMDB storage temporarily commented out due to compilation issues
// Will be re-implemented with a working LMDB binding

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(C, packed)]
pub struct TileHeaderV1 {
    pub magic: u32,        // 'DTX1'
    pub version: u16,      // 1
    pub tile_size: u16,    // e.g., 512
    pub lod: u16,          // LOD level
    pub reserved: u16,
    pub tile_x: u64,       // tile index in x
    pub tile_y: u64,       // tile index in y
    pub bins_offset: u64,  // start of bin payload
    pub bins_len: u32,     // bytes of bin payload (uncompressed)
    pub comp_len: u32,     // bytes after zstd
    pub checksum: u64,     // xxhash64 of uncompressed payload
}

const TILE_MAGIC: u32 = u32::from_le_bytes(*b"DTX1");
const TILE_VERSION: u16 = 1;

pub struct Tile {
    pub coord: TileCoord,
    pub bins: Vec<BinData>,
    pub tile_size: u32,
}

impl Tile {
    pub fn new(coord: TileCoord, tile_size: u32) -> Self {
        let bin_count = (tile_size * tile_size) as usize;
        Self {
            coord,
            bins: vec![BinData::default(); bin_count],
            tile_size,
        }
    }

    pub fn get_bin(&self, x: u32, y: u32) -> Option<&BinData> {
        if x >= self.tile_size || y >= self.tile_size {
            return None;
        }
        let index = (y * self.tile_size + x) as usize;
        self.bins.get(index)
    }

    pub fn get_bin_mut(&mut self, x: u32, y: u32) -> Option<&mut BinData> {
        if x >= self.tile_size || y >= self.tile_size {
            return None;
        }
        let index = (y * self.tile_size + x) as usize;
        self.bins.get_mut(index)
    }

    pub fn add_alignment(&mut self, x: u32, y: u32, record: &PafRecord) {
        if let Some(bin) = self.get_bin_mut(x, y) {
            bin.count = bin.count.saturating_add(1);
            bin.sum_len = bin.sum_len.saturating_add(record.alignment_len as u32);
            
            let identity_scaled = (record.identity() * 1_000_000.0) as u32;
            bin.sum_identity = bin.sum_identity.saturating_add(identity_scaled);
            
            match record.strand {
                Strand::Forward => bin.strand_balance += 1,
                Strand::Reverse => bin.strand_balance -= 1,
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bins.iter().all(|bin| bin.count == 0)
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let bins_data = bytemuck::cast_slice(&self.bins);
        let compressed = zstd::encode_all(bins_data, 3)?;

        let header = TileHeaderV1 {
            magic: TILE_MAGIC,
            version: TILE_VERSION,
            tile_size: self.tile_size as u16,
            lod: self.coord.lod,
            reserved: 0,
            tile_x: self.coord.tile_x,
            tile_y: self.coord.tile_y,
            bins_offset: std::mem::size_of::<TileHeaderV1>() as u64,
            bins_len: bins_data.len() as u32,
            comp_len: compressed.len() as u32,
            checksum: xxhash_rust::xxh64::xxh64(bins_data, 0),
        };

        // Manual serialization to avoid bytemuck padding issues
        let mut result = Vec::new();
        result.extend_from_slice(&header.magic.to_le_bytes());
        result.extend_from_slice(&header.version.to_le_bytes());
        result.extend_from_slice(&header.tile_size.to_le_bytes());
        result.extend_from_slice(&header.lod.to_le_bytes());
        result.extend_from_slice(&header.reserved.to_le_bytes());
        result.extend_from_slice(&header.tile_x.to_le_bytes());
        result.extend_from_slice(&header.tile_y.to_le_bytes());
        result.extend_from_slice(&header.bins_offset.to_le_bytes());
        result.extend_from_slice(&header.bins_len.to_le_bytes());
        result.extend_from_slice(&header.comp_len.to_le_bytes());
        result.extend_from_slice(&header.checksum.to_le_bytes());
        result.extend_from_slice(&compressed);
        Ok(result)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < std::mem::size_of::<TileHeaderV1>() {
            return Err(anyhow::anyhow!("Tile data too short"));
        }

        // Manual deserialization due to bytemuck issues with padding
        if data.len() < 36 { // Minimum size for header
            return Err(anyhow::anyhow!("Tile data too short"));
        }
        
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let version = u16::from_le_bytes([data[4], data[5]]);
        let tile_size = u16::from_le_bytes([data[6], data[7]]);
        let lod = u16::from_le_bytes([data[8], data[9]]);
        let tile_x = u64::from_le_bytes([data[12], data[13], data[14], data[15], data[16], data[17], data[18], data[19]]);
        let tile_y = u64::from_le_bytes([data[20], data[21], data[22], data[23], data[24], data[25], data[26], data[27]]);
        let bins_offset = u64::from_le_bytes([data[28], data[29], data[30], data[31], data[32], data[33], data[34], data[35]]);
        let checksum = if data.len() >= 44 {
            u64::from_le_bytes([data[36], data[37], data[38], data[39], data[40], data[41], data[42], data[43]])
        } else {
            0
        };
        
        if magic != TILE_MAGIC {
            return Err(anyhow::anyhow!("Invalid tile magic"));
        }

        if version != TILE_VERSION {
            return Err(anyhow::anyhow!("Unsupported tile version: {}", version));
        }

        let compressed_data = &data[bins_offset as usize..];
        let decompressed = zstd::decode_all(compressed_data)?;

        // Verify checksum
        let computed_checksum = xxhash_rust::xxh64::xxh64(&decompressed, 0);
        if computed_checksum != checksum && checksum != 0 {
            return Err(anyhow::anyhow!("Tile checksum mismatch"));
        }

        // Manual deserialization of bins
        let bin_size = std::mem::size_of::<BinData>();
        let num_bins = decompressed.len() / bin_size;
        let mut bins = Vec::with_capacity(num_bins);
        
        for i in 0..num_bins {
            let start = i * bin_size;
            if start + bin_size <= decompressed.len() {
                let count = u32::from_le_bytes([
                    decompressed[start], decompressed[start+1], 
                    decompressed[start+2], decompressed[start+3]
                ]);
                let sum_len = u32::from_le_bytes([
                    decompressed[start+4], decompressed[start+5], 
                    decompressed[start+6], decompressed[start+7]
                ]);
                let sum_identity = u32::from_le_bytes([
                    decompressed[start+8], decompressed[start+9], 
                    decompressed[start+10], decompressed[start+11]
                ]);
                let strand_balance = i32::from_le_bytes([
                    decompressed[start+12], decompressed[start+13], 
                    decompressed[start+14], decompressed[start+15]
                ]);
                
                bins.push(BinData {
                    count,
                    sum_len,
                    sum_identity,
                    strand_balance,
                });
            }
        }
        
        let coord = TileCoord {
            lod,
            tile_x,
            tile_y,
        };

        Ok(Tile {
            coord,
            bins,
            tile_size: tile_size as u32,
        })
    }
}

pub struct TileBuilder {
    grid: GridParams,
    tiles: HashMap<TileCoord, Tile>,
}

impl TileBuilder {
    pub fn new(grid: GridParams) -> Self {
        Self {
            grid,
            tiles: HashMap::new(),
        }
    }

    pub fn add_alignment(&mut self, record: &PafRecord) {
        // Convert PAF coordinates to genomic positions
        let ref_start = record.target_start;
        let ref_end = record.target_end;
        let qry_start = record.query_start;
        let qry_end = record.query_end;

        // Use Bresenham-like algorithm to traverse the line segment
        self.traverse_segment(ref_start, ref_end, qry_start, qry_end, record);
    }

    fn traverse_segment(&mut self, ref_start: GenomicPos, ref_end: GenomicPos, 
                       qry_start: GenomicPos, qry_end: GenomicPos, record: &PafRecord) {
        let ref_len = ref_end.saturating_sub(ref_start) as i64;
        let qry_len = qry_end.saturating_sub(qry_start) as i64;
        
        let steps = ref_len.max(qry_len).max(1);
        
        for i in 0..=steps {
            let t = if steps > 0 { i as f64 / steps as f64 } else { 0.0 };
            
            let ref_pos = ref_start + (ref_len as f64 * t) as GenomicPos;
            let qry_pos = qry_start + (qry_len as f64 * t) as GenomicPos;
            
            let (tile_coord, bin_x, bin_y) = self.grid.pos_to_tile_bin(ref_pos, qry_pos);
            
            let tile = self.tiles.entry(tile_coord).or_insert_with(|| {
                Tile::new(tile_coord, self.grid.tile_size)
            });
            
            tile.add_alignment(bin_x, bin_y, record);
        }
    }

    pub fn build_tiles(mut self) -> Vec<Tile> {
        // Remove empty tiles to keep storage sparse
        self.tiles.retain(|_, tile| !tile.is_empty());
        self.tiles.into_values().collect()
    }

    pub fn get_tile(&self, coord: &TileCoord) -> Option<&Tile> {
        self.tiles.get(coord)
    }
}

pub trait TileStorage {
    fn store_tile(&mut self, tile: &Tile) -> Result<()>;
    fn load_tile(&self, coord: &TileCoord) -> Result<Option<Tile>>;
    fn list_tiles(&self, lod: Option<LodLevel>) -> Result<Vec<TileCoord>>;
    fn delete_tile(&mut self, coord: &TileCoord) -> Result<()>;
}

// LMDB storage implementation temporarily disabled
// Will be re-implemented with a working LMDB binding

pub struct InMemoryTileStorage {
    tiles: HashMap<u64, Vec<u8>>,
}

impl InMemoryTileStorage {
    pub fn new() -> Self {
        Self {
            tiles: HashMap::new(),
        }
    }
}

impl TileStorage for InMemoryTileStorage {
    fn store_tile(&mut self, tile: &Tile) -> Result<()> {
        let key = encode_tile_key(tile.coord);
        let value = tile.serialize()?;
        self.tiles.insert(key, value);
        Ok(())
    }

    fn load_tile(&self, coord: &TileCoord) -> Result<Option<Tile>> {
        let key = encode_tile_key(*coord);
        if let Some(data) = self.tiles.get(&key) {
            Ok(Some(Tile::deserialize(data)?))
        } else {
            Ok(None)
        }
    }

    fn list_tiles(&self, lod: Option<LodLevel>) -> Result<Vec<TileCoord>> {
        let mut tiles = Vec::new();
        for &key in self.tiles.keys() {
            let coord = decode_tile_key(key);
            if lod.is_none() || lod == Some(coord.lod) {
                tiles.push(coord);
            }
        }
        Ok(tiles)
    }

    fn delete_tile(&mut self, coord: &TileCoord) -> Result<()> {
        let key = encode_tile_key(*coord);
        self.tiles.remove(&key);
        Ok(())
    }
}