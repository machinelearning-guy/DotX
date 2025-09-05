/*!
# Level of Detail (LOD) Management

Implements a three-tier LOD system for optimal rendering performance:
- Overview: Density heatmaps for structure visualization at low zoom
- Mid-Zoom: Polyline segments showing chain connectivity  
- Deep-Zoom: Individual instanced points with full detail
*/

use crate::Viewport;

/// Level of detail enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LodLevel {
    /// Overview level - density heatmap rendering
    Overview,
    /// Mid-zoom level - polyline segment rendering  
    MidZoom,
    /// Deep-zoom level - instanced point rendering
    DeepZoom,
}

/// LOD management system
#[cfg(feature = "webgpu")]
mod gpu_lod {
    use std::sync::Arc;
    use crate::Viewport;
    use super::LodLevel;

    pub struct LodManager {
        device: Arc<wgpu::Device>,
        overview_threshold: f32,
        mid_zoom_threshold: f32,
        density_texture: Option<wgpu::Texture>,
        density_bind_group: Option<wgpu::BindGroup>,
    }

    impl LodManager {
        pub fn new(device: Arc<wgpu::Device>) -> Self {
            Self {
                device,
                overview_threshold: 8.0,
                mid_zoom_threshold: 16.0,
                density_texture: None,
                density_bind_group: None,
            }
        }

        pub fn determine_lod_level(&self, viewport: &Viewport) -> LodLevel {
            if viewport.zoom_level < self.overview_threshold {
                LodLevel::Overview
            } else if viewport.zoom_level < self.mid_zoom_threshold {
                LodLevel::MidZoom
            } else {
                LodLevel::DeepZoom
            }
        }
    }
}

#[cfg(feature = "webgpu")]
pub use gpu_lod::LodManager;
