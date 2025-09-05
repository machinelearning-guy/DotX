/*!
# Quadtree Tiling System

Implements a hierarchical quadtree tiling system for efficient spatial organization
and culling of genomic anchors. Supports dynamic tile loading and unloading based
on viewport changes.
*/

use std::collections::HashMap;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use crate::{Viewport, LodLevel};

/// Unique identifier for a tile
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId {
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub fn new(level: u8, x: u32, y: u32) -> Self {
        Self { level, x, y }
    }

    /// Get parent tile ID
    pub fn parent(&self) -> Option<TileId> {
        if self.level == 0 {
            None
        } else {
            Some(TileId::new(self.level - 1, self.x / 2, self.y / 2))
        }
    }

    /// Get child tile IDs
    pub fn children(&self) -> [TileId; 4] {
        let level = self.level + 1;
        let x = self.x * 2;
        let y = self.y * 2;
        [
            TileId::new(level, x, y),
            TileId::new(level, x + 1, y),
            TileId::new(level, x, y + 1),
            TileId::new(level, x + 1, y + 1),
        ]
    }
}

/// Spatial bounds for a tile
#[derive(Debug, Clone, Copy)]
pub struct TileBounds {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl TileBounds {
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        Self { x_min, x_max, y_min, y_max }
    }

    pub fn intersects(&self, other: &TileBounds) -> bool {
        self.x_min <= other.x_max
            && self.x_max >= other.x_min
            && self.y_min <= other.y_max
            && self.y_max >= other.y_min
    }

    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.x_min && x <= self.x_max && y >= self.y_min && y <= self.y_max
    }

    pub fn width(&self) -> f64 {
        self.x_max - self.x_min
    }

    pub fn height(&self) -> f64 {
        self.y_max - self.y_min
    }
}

/// A tile containing genomic anchors and rendering data
#[derive(Debug)]
pub struct Tile {
    pub id: TileId,
    pub bounds: TileBounds,
    pub anchors: Vec<dotx_core::types::Anchor>,
    pub lod_level: LodLevel,
    
    // GPU resources
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub instance_buffer: Option<wgpu::Buffer>,
    pub density_data: Option<Vec<f32>>,
    
    // Metadata
    pub anchor_count: usize,
    pub last_accessed: std::time::Instant,
    pub is_loaded: bool,
}

impl Tile {
    pub fn new(id: TileId, bounds: TileBounds, lod_level: LodLevel) -> Self {
        Self {
            id,
            bounds,
            anchors: Vec::new(),
            lod_level,
            vertex_buffer: None,
            index_buffer: None,
            instance_buffer: None,
            density_data: None,
            anchor_count: 0,
            last_accessed: std::time::Instant::now(),
            is_loaded: false,
        }
    }

    /// Add anchors to this tile
    pub fn add_anchors(&mut self, anchors: Vec<dotx_core::types::Anchor>) {
        self.anchor_count = anchors.len();
        self.anchors = anchors;
        self.last_accessed = std::time::Instant::now();
    }

    /// Check if tile needs to be updated based on LOD level
    pub fn needs_update(&self, new_lod: LodLevel) -> bool {
        self.lod_level != new_lod || !self.is_loaded
    }

    /// Get tile bounds from viewport at specified level
    pub fn bounds_from_viewport(viewport: &Viewport, level: u8, x: u32, y: u32) -> TileBounds {
        let tiles_per_level = 1u32 << level;
        let tile_width = (viewport.x_max - viewport.x_min) / tiles_per_level as f64;
        let tile_height = (viewport.y_max - viewport.y_min) / tiles_per_level as f64;
        
        let x_min = viewport.x_min + (x as f64 * tile_width);
        let x_max = x_min + tile_width;
        let y_min = viewport.y_min + (y as f64 * tile_height);
        let y_max = y_min + tile_height;
        
        TileBounds::new(x_min, x_max, y_min, y_max)
    }
}

/// Tile management system with quadtree organization
pub struct TileManager {
    tiles: Arc<DashMap<TileId, Arc<RwLock<Tile>>>>,
    root_bounds: TileBounds,
    max_level: u8,
    tile_cache_size: usize,
    
    // Performance metrics
    cache_hits: Arc<std::sync::atomic::AtomicU64>,
    cache_misses: Arc<std::sync::atomic::AtomicU64>,
}

impl TileManager {
    /// Create a new tile manager
    pub fn new() -> Self {
        Self {
            tiles: Arc::new(DashMap::new()),
            root_bounds: TileBounds::new(0.0, 1.0, 0.0, 1.0),
            max_level: 16, // Support up to 65K x 65K tiles
            tile_cache_size: 1000,
            cache_hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cache_misses: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Set the root bounds for the tile system
    pub fn set_root_bounds(&mut self, bounds: TileBounds) {
        self.root_bounds = bounds;
    }

    /// Update tiles based on current viewport
    pub fn update_tiles(
        &self,
        viewport: &Viewport,
        anchors: &[dotx_core::types::Anchor],
    ) -> anyhow::Result<()> {
        // Determine tiles needed for viewport
        let visible_tiles = self.get_visible_tiles(viewport)?;
        
        // Load/update visible tiles
        for tile_id in visible_tiles {
            let tile_bounds = self.get_tile_bounds(tile_id);
            
            // Filter anchors for this tile
            let tile_anchors: Vec<_> = anchors
                .iter()
                .filter(|anchor| {
                    let x = anchor.target_start as f64;
                    let y = anchor.query_start as f64;
                    tile_bounds.contains_point(x, y)
                })
                .cloned()
                .collect();

            // Get or create tile
            let tile = self.get_or_create_tile(tile_id, tile_bounds);
            
            // Update tile data
            {
                let mut tile_lock = tile.write();
                if !tile_anchors.is_empty() {
                    tile_lock.add_anchors(tile_anchors);
                }
            }
        }

        // Clean up old tiles
        self.cleanup_unused_tiles();

        Ok(())
    }

    /// Get visible tile IDs for the given viewport
    fn get_visible_tiles(&self, viewport: &Viewport) -> anyhow::Result<Vec<TileId>> {
        let mut visible_tiles = Vec::new();
        
        // Determine appropriate level based on zoom
        let level = self.calculate_tile_level(viewport);
        let tiles_per_axis = 1u32 << level;
        
        // Calculate tile indices that intersect viewport
        let tile_width = (self.root_bounds.width()) / tiles_per_axis as f64;
        let tile_height = (self.root_bounds.height()) / tiles_per_axis as f64;
        
        let start_x = ((viewport.x_min - self.root_bounds.x_min) / tile_width).floor() as u32;
        let end_x = ((viewport.x_max - self.root_bounds.x_min) / tile_width).ceil() as u32;
        let start_y = ((viewport.y_min - self.root_bounds.y_min) / tile_height).floor() as u32;
        let end_y = ((viewport.y_max - self.root_bounds.y_min) / tile_height).ceil() as u32;
        
        for y in start_y..=end_y.min(tiles_per_axis - 1) {
            for x in start_x..=end_x.min(tiles_per_axis - 1) {
                visible_tiles.push(TileId::new(level, x, y));
            }
        }
        
        Ok(visible_tiles)
    }

    /// Calculate appropriate tile level for viewport
    fn calculate_tile_level(&self, viewport: &Viewport) -> u8 {
        let zoom_level = viewport.zoom_level as u8;
        (zoom_level / 2).min(self.max_level)
    }

    /// Get tile bounds for a given tile ID
    fn get_tile_bounds(&self, tile_id: TileId) -> TileBounds {
        let tiles_per_axis = 1u32 << tile_id.level;
        let tile_width = self.root_bounds.width() / tiles_per_axis as f64;
        let tile_height = self.root_bounds.height() / tiles_per_axis as f64;
        
        let x_min = self.root_bounds.x_min + (tile_id.x as f64 * tile_width);
        let x_max = x_min + tile_width;
        let y_min = self.root_bounds.y_min + (tile_id.y as f64 * tile_height);
        let y_max = y_min + tile_height;
        
        TileBounds::new(x_min, x_max, y_min, y_max)
    }

    /// Get or create a tile
    fn get_or_create_tile(&self, tile_id: TileId, bounds: TileBounds) -> Arc<RwLock<Tile>> {
        if let Some(existing_tile) = self.tiles.get(&tile_id) {
            self.cache_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            existing_tile.clone()
        } else {
            self.cache_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let tile = Arc::new(RwLock::new(Tile::new(tile_id, bounds, LodLevel::Overview)));
            self.tiles.insert(tile_id, tile.clone());
            tile
        }
    }

    /// Clean up unused tiles to maintain cache size
    fn cleanup_unused_tiles(&self) {
        if self.tiles.len() <= self.tile_cache_size {
            return;
        }

        let current_time = std::time::Instant::now();
        let max_age = std::time::Duration::from_secs(30); // 30 second TTL
        
        let mut to_remove = Vec::new();
        
        for entry in self.tiles.iter() {
            let tile_guard = entry.value().read();
            if current_time.duration_since(tile_guard.last_accessed) > max_age {
                to_remove.push(*entry.key());
            }
        }
        
        // Remove oldest tiles
        for tile_id in to_remove {
            self.tiles.remove(&tile_id);
        }
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> TileStats {
        let hits = self.cache_hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.cache_misses.load(std::sync::atomic::Ordering::Relaxed);
        let hit_rate = if hits + misses > 0 {
            hits as f64 / (hits + misses) as f64
        } else {
            0.0
        };

        TileStats {
            total_tiles: self.tiles.len(),
            cache_hits: hits,
            cache_misses: misses,
            hit_rate,
        }
    }

    /// Get all loaded tiles
    pub fn get_loaded_tiles(&self) -> Vec<Arc<RwLock<Tile>>> {
        self.tiles.iter().map(|entry| entry.value().clone()).collect()
    }
}

/// Tile system performance statistics
#[derive(Debug)]
pub struct TileStats {
    pub total_tiles: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub hit_rate: f64,
}