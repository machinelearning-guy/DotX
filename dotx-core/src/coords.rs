use crate::types::*;
// Morton encoding temporarily simplified due to API changes

pub const TILE_SIZE: u32 = 512;
pub const MAX_LOD: LodLevel = 38;

#[derive(Debug, Clone)]
pub struct GridParams {
    pub lod: LodLevel,
    pub bin_size: u64,
    pub ref_genome: GenomeInfo,
    pub qry_genome: GenomeInfo,
    pub tile_size: u32,
}

impl GridParams {
    pub fn new(lod: LodLevel, ref_genome: GenomeInfo, qry_genome: GenomeInfo) -> Self {
        // Prevent overflow by limiting LOD to reasonable values
        let safe_lod = lod.min(MAX_LOD);
        let bin_size = if safe_lod >= 64 {
            // Prevent overflow for very large LOD values
            u64::MAX
        } else {
            1u64 << safe_lod
        };
        Self {
            lod: safe_lod,
            bin_size,
            ref_genome,
            qry_genome,
            tile_size: TILE_SIZE,
        }
    }

    pub fn pos_to_bin(&self, pos: GenomicPos) -> u64 {
        if self.bin_size == 0 {
            0
        } else {
            pos / self.bin_size
        }
    }

    pub fn bin_to_pos(&self, bin: u64) -> GenomicPos {
        // Use saturating multiplication to prevent overflow
        bin.saturating_mul(self.bin_size)
    }

    pub fn pos_to_tile_bin(&self, ref_pos: GenomicPos, qry_pos: GenomicPos) -> (TileCoord, u32, u32) {
        let ref_bin = self.pos_to_bin(ref_pos);
        let qry_bin = self.pos_to_bin(qry_pos);
        
        let tile_x = ref_bin / self.tile_size as u64;
        let tile_y = qry_bin / self.tile_size as u64;
        
        let bin_x = (ref_bin % self.tile_size as u64) as u32;
        let bin_y = (qry_bin % self.tile_size as u64) as u32;

        let tile_coord = TileCoord {
            lod: self.lod,
            tile_x,
            tile_y,
        };

        (tile_coord, bin_x, bin_y)
    }

    pub fn tile_to_genomic_bounds(&self, tile_coord: TileCoord) -> (GenomicInterval, GenomicInterval) {
        // Use saturating operations to prevent overflow
        let tile_size_u64 = self.tile_size as u64;
        let ref_start = tile_coord.tile_x.saturating_mul(tile_size_u64).saturating_mul(self.bin_size);
        let ref_end = ref_start.saturating_add(tile_size_u64.saturating_mul(self.bin_size));
        
        let qry_start = tile_coord.tile_y.saturating_mul(tile_size_u64).saturating_mul(self.bin_size);
        let qry_end = qry_start.saturating_add(tile_size_u64.saturating_mul(self.bin_size));

        // Find which contigs these positions fall into
        let ref_interval = self.find_contig_interval(&self.ref_genome, ref_start, ref_end);
        let qry_interval = self.find_contig_interval(&self.qry_genome, qry_start, qry_end);

        (ref_interval, qry_interval)
    }

    fn find_contig_interval(&self, genome: &GenomeInfo, start: GenomicPos, end: GenomicPos) -> GenomicInterval {
        // For now, assume single contig or use global coordinates
        // TODO: Handle multi-contig properly
        GenomicInterval {
            contig_id: 0,
            start: start.min(genome.total_length.saturating_sub(1)),
            end: end.min(genome.total_length),
        }
    }
}

// Simple hash-based encoding temporarily (will be replaced with proper Morton encoding)
pub fn encode_tile_key(tile_coord: TileCoord) -> u64 {
    // Use saturating arithmetic to prevent overflow and ensure deterministic results
    let mut key = tile_coord.lod as u64;
    key = key.saturating_mul(1000000).saturating_add(tile_coord.tile_x);
    key = key.saturating_mul(1000000).saturating_add(tile_coord.tile_y);
    key
}

pub fn decode_tile_key(key: u64) -> TileCoord {
    // Reverse the simple hash (approximate - may not be perfect for all keys)
    let tile_y = key % 1000000;
    let remaining = key / 1000000;
    let tile_x = remaining % 1000000;
    let lod = (remaining / 1000000) as LodLevel;
    
    TileCoord { lod, tile_x, tile_y }
}

pub fn tile_intersects_region(
    tile_coord: TileCoord,
    grid: &GridParams,
    ref_region: &GenomicInterval,
    qry_region: &GenomicInterval,
) -> bool {
    let (tile_ref_bounds, tile_qry_bounds) = grid.tile_to_genomic_bounds(tile_coord);
    
    // Check if intervals overlap
    intervals_overlap(&tile_ref_bounds, ref_region) && 
    intervals_overlap(&tile_qry_bounds, qry_region)
}

fn intervals_overlap(a: &GenomicInterval, b: &GenomicInterval) -> bool {
    a.contig_id == b.contig_id && 
    a.start < b.end && 
    b.start < a.end
}