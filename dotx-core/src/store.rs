//! Binary store (.dotxdb) implementation
//!
//! This module implements the binary storage format for DOTx data as specified in the plan:
//! - Header { magic="DOTX", version, build_meta }  
//! - Meta   { samples, contigs, lengths, index offsets }
//! - Anchors { delta-encoded coords, strand bits, engine tags }
//! - Chains  { chain index -> ranges in Anchors }
//! - Tiles   { quadtree tile index -> anchor spans / density rasters }
//! - Verify  { optional: ROI results (identity/indels) keyed by tile }

use crate::types::{Anchor, Strand};
use crate::tiles::DensityTile;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write, Seek, SeekFrom};
use std::path::Path;
use thiserror::Error;

/// Magic bytes for the DotX binary format
const DOTX_MAGIC: &[u8] = b"DOTX";

/// Current binary format version
const DOTX_VERSION: u32 = 1;

/// Errors that can occur during store operations
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid magic bytes: expected DOTX")]
    InvalidMagic,
    
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u32),
    
    #[error("Compression error: {0}")]
    Compression(String),
    
    #[error("Decompression error: {0}")]
    Decompression(String),
    
    #[error("Index error: {0}")]
    Index(String),
    
    #[error("Data corruption: {0}")]
    Corruption(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

/// Header section of the binary store
#[derive(Debug, Clone)]
pub struct Header {
    pub magic: [u8; 4],
    pub version: u32,
    pub build_timestamp: u64,
    pub build_metadata: String,
    pub flags: u32,
}

impl Header {
    pub fn new() -> Self {
        Self {
            magic: *b"DOTX",
            version: DOTX_VERSION,
            build_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            build_metadata: format!("dotx-core-{}", env!("CARGO_PKG_VERSION")),
            flags: 0,
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> StoreResult<()> {
        writer.write_all(&self.magic)?;
        writer.write_u32::<LittleEndian>(self.version)?;
        writer.write_u64::<LittleEndian>(self.build_timestamp)?;
        
        let metadata_bytes = self.build_metadata.as_bytes();
        writer.write_u32::<LittleEndian>(metadata_bytes.len() as u32)?;
        writer.write_all(metadata_bytes)?;
        
        writer.write_u32::<LittleEndian>(self.flags)?;
        Ok(())
    }

    fn read<R: Read>(reader: &mut R) -> StoreResult<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        
        if &magic != DOTX_MAGIC {
            return Err(StoreError::InvalidMagic);
        }
        
        let version = reader.read_u32::<LittleEndian>()?;
        if version != DOTX_VERSION {
            return Err(StoreError::UnsupportedVersion(version));
        }
        
        let build_timestamp = reader.read_u64::<LittleEndian>()?;
        
        let metadata_len = reader.read_u32::<LittleEndian>()? as usize;
        let mut metadata_bytes = vec![0u8; metadata_len];
        reader.read_exact(&mut metadata_bytes)?;
        let build_metadata = String::from_utf8_lossy(&metadata_bytes).to_string();
        
        let flags = reader.read_u32::<LittleEndian>()?;
        
        Ok(Self {
            magic,
            version,
            build_timestamp,
            build_metadata,
            flags,
        })
    }
}

/// Metadata about sequences and contigs
#[derive(Debug, Clone)]
pub struct ContigInfo {
    pub name: String,
    pub length: u64,
    pub checksum: Option<String>,
}

/// Meta section containing sequence metadata and section offsets
#[derive(Debug, Clone)]
pub struct Meta {
    pub query_contigs: Vec<ContigInfo>,
    pub target_contigs: Vec<ContigInfo>,
    pub anchors_offset: u64,
    pub anchors_size: u64,
    pub chains_offset: u64,
    pub chains_size: u64,
    pub tiles_offset: u64,
    pub tiles_size: u64,
    pub verify_offset: u64,
    pub verify_size: u64,
}

impl Meta {
    fn write<W: Write>(&self, writer: &mut W) -> StoreResult<()> {
        // Write query contigs
        writer.write_u32::<LittleEndian>(self.query_contigs.len() as u32)?;
        for contig in &self.query_contigs {
            let name_bytes = contig.name.as_bytes();
            writer.write_u32::<LittleEndian>(name_bytes.len() as u32)?;
            writer.write_all(name_bytes)?;
            writer.write_u64::<LittleEndian>(contig.length)?;
            
            // Write checksum if present
            if let Some(ref checksum) = contig.checksum {
                writer.write_u8(1)?; // has checksum
                let checksum_bytes = checksum.as_bytes();
                writer.write_u32::<LittleEndian>(checksum_bytes.len() as u32)?;
                writer.write_all(checksum_bytes)?;
            } else {
                writer.write_u8(0)?; // no checksum
            }
        }
        
        // Write target contigs
        writer.write_u32::<LittleEndian>(self.target_contigs.len() as u32)?;
        for contig in &self.target_contigs {
            let name_bytes = contig.name.as_bytes();
            writer.write_u32::<LittleEndian>(name_bytes.len() as u32)?;
            writer.write_all(name_bytes)?;
            writer.write_u64::<LittleEndian>(contig.length)?;
            
            if let Some(ref checksum) = contig.checksum {
                writer.write_u8(1)?;
                let checksum_bytes = checksum.as_bytes();
                writer.write_u32::<LittleEndian>(checksum_bytes.len() as u32)?;
                writer.write_all(checksum_bytes)?;
            } else {
                writer.write_u8(0)?;
            }
        }
        
        // Write section offsets
        writer.write_u64::<LittleEndian>(self.anchors_offset)?;
        writer.write_u64::<LittleEndian>(self.anchors_size)?;
        writer.write_u64::<LittleEndian>(self.chains_offset)?;
        writer.write_u64::<LittleEndian>(self.chains_size)?;
        writer.write_u64::<LittleEndian>(self.tiles_offset)?;
        writer.write_u64::<LittleEndian>(self.tiles_size)?;
        writer.write_u64::<LittleEndian>(self.verify_offset)?;
        writer.write_u64::<LittleEndian>(self.verify_size)?;
        
        Ok(())
    }
    
    fn read<R: Read>(reader: &mut R) -> StoreResult<Self> {
        // Read query contigs
        let query_count = reader.read_u32::<LittleEndian>()? as usize;
        let mut query_contigs = Vec::with_capacity(query_count);
        
        for _ in 0..query_count {
            let name_len = reader.read_u32::<LittleEndian>()? as usize;
            let mut name_bytes = vec![0u8; name_len];
            reader.read_exact(&mut name_bytes)?;
            let name = String::from_utf8_lossy(&name_bytes).to_string();
            
            let length = reader.read_u64::<LittleEndian>()?;
            
            let has_checksum = reader.read_u8()? != 0;
            let checksum = if has_checksum {
                let checksum_len = reader.read_u32::<LittleEndian>()? as usize;
                let mut checksum_bytes = vec![0u8; checksum_len];
                reader.read_exact(&mut checksum_bytes)?;
                Some(String::from_utf8_lossy(&checksum_bytes).to_string())
            } else {
                None
            };
            
            query_contigs.push(ContigInfo {
                name,
                length,
                checksum,
            });
        }
        
        // Read target contigs
        let target_count = reader.read_u32::<LittleEndian>()? as usize;
        let mut target_contigs = Vec::with_capacity(target_count);
        
        for _ in 0..target_count {
            let name_len = reader.read_u32::<LittleEndian>()? as usize;
            let mut name_bytes = vec![0u8; name_len];
            reader.read_exact(&mut name_bytes)?;
            let name = String::from_utf8_lossy(&name_bytes).to_string();
            
            let length = reader.read_u64::<LittleEndian>()?;
            
            let has_checksum = reader.read_u8()? != 0;
            let checksum = if has_checksum {
                let checksum_len = reader.read_u32::<LittleEndian>()? as usize;
                let mut checksum_bytes = vec![0u8; checksum_len];
                reader.read_exact(&mut checksum_bytes)?;
                Some(String::from_utf8_lossy(&checksum_bytes).to_string())
            } else {
                None
            };
            
            target_contigs.push(ContigInfo {
                name,
                length,
                checksum,
            });
        }
        
        // Read section offsets
        let anchors_offset = reader.read_u64::<LittleEndian>()?;
        let anchors_size = reader.read_u64::<LittleEndian>()?;
        let chains_offset = reader.read_u64::<LittleEndian>()?;
        let chains_size = reader.read_u64::<LittleEndian>()?;
        let tiles_offset = reader.read_u64::<LittleEndian>()?;
        let tiles_size = reader.read_u64::<LittleEndian>()?;
        let verify_offset = reader.read_u64::<LittleEndian>()?;
        let verify_size = reader.read_u64::<LittleEndian>()?;
        
        Ok(Self {
            query_contigs,
            target_contigs,
            anchors_offset,
            anchors_size,
            chains_offset,
            chains_size,
            tiles_offset,
            tiles_size,
            verify_offset,
            verify_size,
        })
    }
}

/// Chain representing a sequence of related anchors
#[derive(Debug, Clone)]
pub struct Chain {
    pub id: u32,
    pub query_contig: String,
    pub target_contig: String,
    pub anchor_start: u32,
    pub anchor_count: u32,
    pub score: f32,
    pub strand: Strand,
}

/// Tile for hierarchical indexing
#[derive(Debug, Clone)]
pub struct Tile {
    pub level: u8,
    pub x: u32,
    pub y: u32,
    pub anchor_start: u32,
    pub anchor_count: u32,
    pub density: f32,
}

/// Verification result for a tile
#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub tile_id: u64,
    pub identity: f32,
    pub insertions: u32,
    pub deletions: u32,
    pub substitutions: u32,
}

/// Main binary store interface
pub struct DotXStore {
    header: Header,
    meta: Meta,
    query_contig_map: HashMap<String, usize>,
    target_contig_map: HashMap<String, usize>,
}

impl DotXStore {
    /// Create a new empty store
    pub fn new() -> Self {
        Self {
            header: Header::new(),
            meta: Meta {
                query_contigs: Vec::new(),
                target_contigs: Vec::new(),
                anchors_offset: 0,
                anchors_size: 0,
                chains_offset: 0,
                chains_size: 0,
                tiles_offset: 0,
                tiles_size: 0,
                verify_offset: 0,
                verify_size: 0,
            },
            query_contig_map: HashMap::new(),
            target_contig_map: HashMap::new(),
        }
    }
    
    /// Add contig information
    pub fn add_query_contig(&mut self, name: String, length: u64, checksum: Option<String>) {
        let index = self.meta.query_contigs.len();
        self.query_contig_map.insert(name.clone(), index);
        self.meta.query_contigs.push(ContigInfo {
            name,
            length,
            checksum,
        });
    }
    
    pub fn add_target_contig(&mut self, name: String, length: u64, checksum: Option<String>) {
        let index = self.meta.target_contigs.len();
        self.target_contig_map.insert(name.clone(), index);
        self.meta.target_contigs.push(ContigInfo {
            name,
            length,
            checksum,
        });
    }
    
    /// Write the store to a file
    pub fn write_to_file<P: AsRef<Path>>(&mut self, path: P, anchors: &[Anchor]) -> StoreResult<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        
        // Write header
        self.header.write(&mut writer)?;
        
        // Calculate section offsets (placeholder for now)
        let header_end = writer.stream_position()?;
        
        // Write meta section (placeholder offsets)
        self.meta.write(&mut writer)?;
        let meta_end = writer.stream_position()?;
        
        // Write anchors section
        self.meta.anchors_offset = meta_end;
        self.write_anchors(&mut writer, anchors)?;
        let anchors_end = writer.stream_position()?;
        self.meta.anchors_size = anchors_end - self.meta.anchors_offset;
        
        // Placeholder for other sections
        self.meta.chains_offset = anchors_end;
        self.meta.chains_size = 0;
        self.meta.tiles_offset = anchors_end;
        self.meta.tiles_size = 0;
        self.meta.verify_offset = anchors_end;
        self.meta.verify_size = 0;
        
        // Go back and update meta with correct offsets
        writer.seek(SeekFrom::Start(header_end))?;
        self.meta.write(&mut writer)?;
        
        writer.flush()?;
        Ok(())
    }

    /// Write the store to a file, including a precomputed tiles section
    pub fn write_to_file_with_tiles<P: AsRef<Path>>(
        &mut self,
        path: P,
        anchors: &[Anchor],
        tiles: &[DensityTile],
    ) -> StoreResult<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Write header
        self.header.write(&mut writer)?;

        // Reserve space for meta
        let header_end = writer.stream_position()?;
        self.meta.write(&mut writer)?;
        let meta_end = writer.stream_position()?;

        // Anchors
        self.meta.anchors_offset = meta_end;
        self.write_anchors(&mut writer, anchors)?;
        let anchors_end = writer.stream_position()?;
        self.meta.anchors_size = anchors_end - self.meta.anchors_offset;

        // Tiles
        self.meta.tiles_offset = anchors_end;
        self.write_tiles(&mut writer, tiles)?;
        let tiles_end = writer.stream_position()?;
        self.meta.tiles_size = tiles_end - self.meta.tiles_offset;

        // Chains/Verify not written in this path
        self.meta.chains_offset = tiles_end;
        self.meta.chains_size = 0;
        self.meta.verify_offset = tiles_end;
        self.meta.verify_size = 0;

        // Backpatch meta
        writer.seek(SeekFrom::Start(header_end))?;
        self.meta.write(&mut writer)?;
        writer.flush()?;
        Ok(())
    }

    /// Write the store to a file, including precomputed tiles and verify section
    pub fn write_to_file_with_tiles_and_verify<P: AsRef<Path>>(
        &mut self,
        path: P,
        anchors: &[Anchor],
        tiles: &[DensityTile],
        verify: &[VerifyResult],
    ) -> StoreResult<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Header and placeholder meta
        self.header.write(&mut writer)?;
        let meta_pos = writer.stream_position()?;
        self.meta.write(&mut writer)?;
        let meta_end = writer.stream_position()?;

        // Anchors
        self.meta.anchors_offset = meta_end;
        self.write_anchors(&mut writer, anchors)?;
        let anchors_end = writer.stream_position()?;
        self.meta.anchors_size = anchors_end - self.meta.anchors_offset;

        // Tiles
        self.meta.tiles_offset = anchors_end;
        self.write_tiles(&mut writer, tiles)?;
        let tiles_end = writer.stream_position()?;
        self.meta.tiles_size = tiles_end - self.meta.tiles_offset;

        // Verify
        self.meta.verify_offset = tiles_end;
        self.write_verify(&mut writer, verify)?;
        let verify_end = writer.stream_position()?;
        self.meta.verify_size = verify_end - self.meta.verify_offset;

        // Chains not written
        self.meta.chains_offset = verify_end;
        self.meta.chains_size = 0;

        // Backpatch meta
        writer.seek(SeekFrom::Start(meta_pos))?;
        self.meta.write(&mut writer)?;
        writer.flush()?;
        Ok(())
    }
    
    /// Read store from file
    pub fn read_from_file<P: AsRef<Path>>(path: P) -> StoreResult<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        
        let header = Header::read(&mut reader)?;
        let meta = Meta::read(&mut reader)?;
        
        // Build contig maps
        let mut query_contig_map = HashMap::new();
        for (i, contig) in meta.query_contigs.iter().enumerate() {
            query_contig_map.insert(contig.name.clone(), i);
        }
        
        let mut target_contig_map = HashMap::new();
        for (i, contig) in meta.target_contigs.iter().enumerate() {
            target_contig_map.insert(contig.name.clone(), i);
        }
        
        Ok(Self {
            header,
            meta,
            query_contig_map,
            target_contig_map,
        })
    }
    
    /// Write anchors section with delta encoding and compression
    fn write_anchors<W: Write + Seek>(&self, writer: &mut W, anchors: &[Anchor]) -> StoreResult<()> {
        let mut buffer = Vec::new();
        
        // Sort anchors by query and target positions for better delta compression
        let mut sorted_anchors = anchors.to_vec();
        sorted_anchors.sort_by(|a, b| {
            a.q.cmp(&b.q)
                .then(a.t.cmp(&b.t))
                .then(a.qs.cmp(&b.qs))
                .then(a.ts.cmp(&b.ts))
        });
        
        buffer.write_u32::<LittleEndian>(sorted_anchors.len() as u32)?;
        
        let mut last_qs = 0u64;
        let mut last_qe = 0u64;
        let mut last_ts = 0u64;
        let mut last_te = 0u64;
        
        for anchor in &sorted_anchors {
            // Delta encode coordinates
            let delta_qs = anchor.qs.wrapping_sub(last_qs);
            let delta_qe = anchor.qe.wrapping_sub(last_qe);
            let delta_ts = anchor.ts.wrapping_sub(last_ts);
            let delta_te = anchor.te.wrapping_sub(last_te);
            
            // Write sequence IDs
            let q_bytes = anchor.q.as_bytes();
            buffer.write_u16::<LittleEndian>(q_bytes.len() as u16)?;
            buffer.write_all(q_bytes)?;
            
            let t_bytes = anchor.t.as_bytes();
            buffer.write_u16::<LittleEndian>(t_bytes.len() as u16)?;
            buffer.write_all(t_bytes)?;
            
            // Write delta-encoded coordinates
            buffer.write_u64::<LittleEndian>(delta_qs)?;
            buffer.write_u64::<LittleEndian>(delta_qe)?;
            buffer.write_u64::<LittleEndian>(delta_ts)?;
            buffer.write_u64::<LittleEndian>(delta_te)?;
            
            // Write strand as bit
            buffer.write_u8(match anchor.strand {
                Strand::Forward => 0,
                Strand::Reverse => 1,
            })?;
            
            // Write optional fields
            if let Some(mapq) = anchor.mapq {
                buffer.write_u8(1)?; // has mapq
                buffer.write_u8(mapq)?;
            } else {
                buffer.write_u8(0)?; // no mapq
            }
            
            if let Some(identity) = anchor.identity {
                buffer.write_u8(1)?; // has identity
                buffer.write_f32::<LittleEndian>(identity)?;
            } else {
                buffer.write_u8(0)?; // no identity
            }
            
            // Write engine tag
            let engine_bytes = anchor.engine_tag.as_bytes();
            buffer.write_u16::<LittleEndian>(engine_bytes.len() as u16)?;
            buffer.write_all(engine_bytes)?;
            
            last_qs = anchor.qs;
            last_qe = anchor.qe;
            last_ts = anchor.ts;
            last_te = anchor.te;
        }
        
        // Compress the buffer
        let compressed = zstd::encode_all(&buffer[..], 3)
            .map_err(|e| StoreError::Compression(e.to_string()))?;
        
        writer.write_u64::<LittleEndian>(compressed.len() as u64)?;
        writer.write_all(&compressed)?;
        
        Ok(())
    }
    
    /// Read anchors section
    pub fn read_anchors<R: Read + Seek>(&self, reader: &mut R) -> StoreResult<Vec<Anchor>> {
        reader.seek(SeekFrom::Start(self.meta.anchors_offset))?;
        
        let compressed_size = reader.read_u64::<LittleEndian>()? as usize;
        let mut compressed_data = vec![0u8; compressed_size];
        reader.read_exact(&mut compressed_data)?;
        
        let decompressed = zstd::decode_all(&compressed_data[..])
            .map_err(|e| StoreError::Decompression(e.to_string()))?;
        
        let mut buffer = std::io::Cursor::new(decompressed);
        let count = buffer.read_u32::<LittleEndian>()? as usize;
        let mut anchors = Vec::with_capacity(count);
        
        let mut last_qs = 0u64;
        let mut last_qe = 0u64;
        let mut last_ts = 0u64;
        let mut last_te = 0u64;
        
        for _ in 0..count {
            // Read sequence IDs
            let q_len = buffer.read_u16::<LittleEndian>()? as usize;
            let mut q_bytes = vec![0u8; q_len];
            buffer.read_exact(&mut q_bytes)?;
            let q = String::from_utf8_lossy(&q_bytes).to_string();
            
            let t_len = buffer.read_u16::<LittleEndian>()? as usize;
            let mut t_bytes = vec![0u8; t_len];
            buffer.read_exact(&mut t_bytes)?;
            let t = String::from_utf8_lossy(&t_bytes).to_string();
            
            // Read delta-encoded coordinates
            let delta_qs = buffer.read_u64::<LittleEndian>()?;
            let delta_qe = buffer.read_u64::<LittleEndian>()?;
            let delta_ts = buffer.read_u64::<LittleEndian>()?;
            let delta_te = buffer.read_u64::<LittleEndian>()?;
            
            let qs = last_qs.wrapping_add(delta_qs);
            let qe = last_qe.wrapping_add(delta_qe);
            let ts = last_ts.wrapping_add(delta_ts);
            let te = last_te.wrapping_add(delta_te);
            
            // Read strand
            let strand = match buffer.read_u8()? {
                0 => Strand::Forward,
                1 => Strand::Reverse,
                _ => return Err(StoreError::Corruption("Invalid strand value".to_string())),
            };
            
            // Read optional fields
            let mapq = if buffer.read_u8()? != 0 {
                Some(buffer.read_u8()?)
            } else {
                None
            };
            
            let identity = if buffer.read_u8()? != 0 {
                Some(buffer.read_f32::<LittleEndian>()?)
            } else {
                None
            };
            
            // Read engine tag
            let engine_len = buffer.read_u16::<LittleEndian>()? as usize;
            let mut engine_bytes = vec![0u8; engine_len];
            buffer.read_exact(&mut engine_bytes)?;
            let engine_tag = String::from_utf8_lossy(&engine_bytes).to_string();
            
            anchors.push(Anchor {
                q,
                t,
                qs,
                qe,
                ts,
                te,
                strand,
                mapq,
                identity,
                engine_tag,
                query_length: None,
                target_length: None,
                residue_matches: None,
                alignment_block_length: None,
                tags: HashMap::new(),
            });
            
            last_qs = qs;
            last_qe = qe;
            last_ts = ts;
            last_te = te;
        }
        
        Ok(anchors)
    }

    /// Write tiles section (compressed)
    fn write_tiles<W: Write + Seek>(&self, writer: &mut W, tiles: &[DensityTile]) -> StoreResult<()> {
        // Binary format:
        // [zstd_len u64][zstd_payload of: count u32, then per-record: level u8, x u32, y u32, count u32, density f32]
        let mut buffer = Vec::new();
        use byteorder::{LittleEndian, WriteBytesExt};

        buffer.write_u32::<LittleEndian>(tiles.len() as u32)?;
        for t in tiles {
            buffer.write_u8(t.level)?;
            buffer.write_u32::<LittleEndian>(t.x)?;
            buffer.write_u32::<LittleEndian>(t.y)?;
            buffer.write_u32::<LittleEndian>(t.count)?;
            buffer.write_f32::<LittleEndian>(t.density)?;
        }

        let compressed = zstd::encode_all(&buffer[..], 3)
            .map_err(|e| StoreError::Compression(e.to_string()))?;
        writer.write_u64::<LittleEndian>(compressed.len() as u64)?;
        writer.write_all(&compressed)?;
        Ok(())
    }

    /// Read tiles section
    pub fn read_tiles<R: Read + Seek>(&self, reader: &mut R) -> StoreResult<Vec<DensityTile>> {
        if self.meta.tiles_size == 0 { return Ok(Vec::new()); }
        reader.seek(SeekFrom::Start(self.meta.tiles_offset))?;
        use byteorder::{LittleEndian, ReadBytesExt};
        let compressed_size = reader.read_u64::<LittleEndian>()? as usize;
        let mut compressed_data = vec![0u8; compressed_size];
        reader.read_exact(&mut compressed_data)?;
        let decompressed = zstd::decode_all(&compressed_data[..])
            .map_err(|e| StoreError::Decompression(e.to_string()))?;
        let mut cursor = std::io::Cursor::new(decompressed);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let level = cursor.read_u8()?;
            let x = cursor.read_u32::<LittleEndian>()?;
            let y = cursor.read_u32::<LittleEndian>()?;
            let c = cursor.read_u32::<LittleEndian>()?;
            let d = cursor.read_f32::<LittleEndian>()?;
            out.push(DensityTile { level, x, y, count: c, density: d });
        }
        Ok(out)
    }

    /// Write verify section (compressed)
    fn write_verify<W: Write + Seek>(&self, writer: &mut W, verify: &[VerifyResult]) -> StoreResult<()> {
        // [zstd_len u64][zstd_payload of: count u32, then per-record: tile_id u64, identity f32, ins u32, del u32, sub u32]
        use byteorder::{LittleEndian, WriteBytesExt};
        let mut buffer = Vec::new();
        buffer.write_u32::<LittleEndian>(verify.len() as u32)?;
        for v in verify {
            buffer.write_u64::<LittleEndian>(v.tile_id)?;
            buffer.write_f32::<LittleEndian>(v.identity)?;
            buffer.write_u32::<LittleEndian>(v.insertions)?;
            buffer.write_u32::<LittleEndian>(v.deletions)?;
            buffer.write_u32::<LittleEndian>(v.substitutions)?;
        }
        let compressed = zstd::encode_all(&buffer[..], 3)
            .map_err(|e| StoreError::Compression(e.to_string()))?;
        writer.write_u64::<LittleEndian>(compressed.len() as u64)?;
        writer.write_all(&compressed)?;
        Ok(())
    }

    /// Read verify section
    pub fn read_verify<R: Read + Seek>(&self, reader: &mut R) -> StoreResult<Vec<VerifyResult>> {
        if self.meta.verify_size == 0 { return Ok(Vec::new()); }
        reader.seek(SeekFrom::Start(self.meta.verify_offset))?;
        use byteorder::{LittleEndian, ReadBytesExt};
        let compressed_size = reader.read_u64::<LittleEndian>()? as usize;
        let mut compressed_data = vec![0u8; compressed_size];
        reader.read_exact(&mut compressed_data)?;
        let decompressed = zstd::decode_all(&compressed_data[..])
            .map_err(|e| StoreError::Decompression(e.to_string()))?;
        let mut cursor = std::io::Cursor::new(decompressed);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let tile_id = cursor.read_u64::<LittleEndian>()?;
            let identity = cursor.read_f32::<LittleEndian>()?;
            let insertions = cursor.read_u32::<LittleEndian>()?;
            let deletions = cursor.read_u32::<LittleEndian>()?;
            let substitutions = cursor.read_u32::<LittleEndian>()?;
            out.push(VerifyResult { tile_id, identity, insertions, deletions, substitutions });
        }
        Ok(out)
    }
    
    /// Get store metadata
    pub fn get_header(&self) -> &Header {
        &self.header
    }
    
    pub fn get_meta(&self) -> &Meta {
        &self.meta
    }
}

impl Default for DotXStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_store_roundtrip() -> StoreResult<()> {
        let mut store = DotXStore::new();
        
        // Add test contigs
        store.add_query_contig("chr1".to_string(), 1000000, None);
        store.add_target_contig("chr2".to_string(), 2000000, None);
        
        // Create test anchors
        let anchors = vec![
            Anchor::new(
                "chr1".to_string(),
                "chr2".to_string(),
                1000, 2000,
                5000, 6000,
                Strand::Forward,
                "test".to_string(),
            ),
            Anchor::new(
                "chr1".to_string(),
                "chr2".to_string(),
                10000, 11000,
                15000, 16000,
                Strand::Reverse,
                "test".to_string(),
            ).with_mapping_quality(60).with_identity(95.5),
        ];
        
        // Write to temporary file
        let temp_file = NamedTempFile::new()?;
        store.write_to_file(temp_file.path(), &anchors)?;
        
        // Read back
        let loaded_store = DotXStore::read_from_file(temp_file.path())?;
        assert_eq!(loaded_store.meta.query_contigs.len(), 1);
        assert_eq!(loaded_store.meta.target_contigs.len(), 1);
        
        // Read anchors back
        let mut file = File::open(temp_file.path())?;
        let loaded_anchors = loaded_store.read_anchors(&mut file)?;
        assert_eq!(loaded_anchors.len(), 2);
        
        // Check first anchor
        assert_eq!(loaded_anchors[0].q, "chr1");
        assert_eq!(loaded_anchors[0].t, "chr2");
        assert_eq!(loaded_anchors[0].qs, 1000);
        assert_eq!(loaded_anchors[0].strand, Strand::Forward);
        
        // Check second anchor
        assert_eq!(loaded_anchors[1].mapq, Some(60));
        assert_eq!(loaded_anchors[1].identity, Some(95.5));
        
        Ok(())
    }

    #[test]
    fn test_store_tiles_verify_roundtrip() -> StoreResult<()> {
        let mut store = DotXStore::new();

        // Minimal anchors
        let anchors = vec![
            Anchor::new(
                "q".to_string(),
                "t".to_string(),
                10, 20,
                30, 40,
                Strand::Forward,
                "test".to_string(),
            ),
        ];

        // Simple tiles
        let tiles = vec![
            DensityTile { level: 0, x: 0, y: 0, count: 5, density: 1.0 },
            DensityTile { level: 1, x: 1, y: 2, count: 3, density: 0.6 },
        ];

        // Verify records
        let verify = vec![
            VerifyResult { tile_id: ((1u64)<<56) | ((1u64)<<28) | 2u64, identity: 98.5, insertions: 1, deletions: 0, substitutions: 2 },
        ];

        let temp = NamedTempFile::new()?;
        store.write_to_file_with_tiles_and_verify(temp.path(), &anchors, &tiles, &verify)?;

        let loaded = DotXStore::read_from_file(temp.path())?;
        let mut f = File::open(temp.path())?;
        let read_tiles = loaded.read_tiles(&mut f)?;
        assert_eq!(read_tiles.len(), tiles.len());

        // Need fresh file handle or seek
        use std::io::Seek;
        f.rewind()?;
        // Read header + meta again to set offsets in loaded, then read verify
        let verify_read = loaded.read_verify(&mut f)?;
        assert_eq!(verify_read.len(), verify.len());
        assert_eq!(verify_read[0].tile_id, verify[0].tile_id);
        assert!((verify_read[0].identity - verify[0].identity).abs() < 1e-3);

        Ok(())
    }
}
