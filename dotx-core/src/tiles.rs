//! Tile building utilities for DOTx (.dotxdb Tiles section)

use crate::types::Anchor;

/// Configuration for building density tiles
#[derive(Debug, Clone, Copy)]
pub struct TileBuildConfig {
    /// Number of LOD levels (0..levels-1)
    pub levels: u8,
    /// Base grid resolution at level 0 (e.g., 64 -> 64x64 at level 0)
    pub base_resolution: u32,
}

impl Default for TileBuildConfig {
    fn default() -> Self {
        Self { levels: 6, base_resolution: 64 }
    }
}

/// A density tile cell with count normalized to [0,1] per-level
#[derive(Debug, Clone)]
pub struct DensityTile {
    pub level: u8,
    pub x: u32,
    pub y: u32,
    pub count: u32,
    pub density: f32,
}

/// Build a simple global density pyramid over all anchors in t (X) vs q (Y)
pub fn build_density_tiles(anchors: &[Anchor], cfg: TileBuildConfig) -> Vec<DensityTile> {
    if anchors.is_empty() { return Vec::new(); }

    // Compute global extents
    let (mut t_min, mut t_max, mut q_min, mut q_max) = (u64::MAX, 0u64, u64::MAX, 0u64);
    for a in anchors {
        t_min = t_min.min(a.ts); t_max = t_max.max(a.te);
        q_min = q_min.min(a.qs); q_max = q_max.max(a.qe);
    }
    if t_min == u64::MAX { t_min = 0; }
    if q_min == u64::MAX { q_min = 0; }
    let t_span = (t_max - t_min).max(1);
    let q_span = (q_max - q_min).max(1);

    let mut out = Vec::new();

    for level in 0..cfg.levels {
        let scale = 1u32 << level;
        let res = cfg.base_resolution * scale;
        let mut counts = vec![0u32; (res as usize) * (res as usize)];

        // binning by anchor start positions
        for a in anchors {
            let nx = ((a.ts.saturating_sub(t_min)) as f64 / t_span as f64).clamp(0.0, 1.0);
            let ny = ((a.qs.saturating_sub(q_min)) as f64 / q_span as f64).clamp(0.0, 1.0);
            let mut ix = (nx * (res as f64)).floor() as i64;
            let mut iy = (ny * (res as f64)).floor() as i64;
            if ix == res as i64 { ix = res as i64 - 1; }
            if iy == res as i64 { iy = res as i64 - 1; }
            if ix >= 0 && iy >= 0 { counts[(iy as usize) * res as usize + (ix as usize)] += 1; }
        }

        let max_count = counts.iter().copied().max().unwrap_or(1).max(1);
        for y in 0..res { for x in 0..res {
            let idx = (y * res + x) as usize;
            let c = counts[idx];
            if c == 0 { continue; }
            out.push(DensityTile {
                level,
                x,
                y,
                count: c,
                density: (c as f32) / (max_count as f32),
            });
        }}
    }

    out
}

