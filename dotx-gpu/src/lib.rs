/*!
# DotX GPU Rendering Pipeline

High-performance GPU-accelerated rendering system for genome visualizations with
Level-of-Detail (LOD) support and instanced rendering for millions of anchors.

## Architecture

The rendering pipeline implements a three-tier LOD system:
1. **Overview Level**: Density heatmaps for high-level structure visualization
2. **Mid-Zoom Level**: Polyline segments showing chain structures  
3. **Deep-Zoom Level**: Individual instanced points with detailed tooltips

## Performance Targets

- ≥60 FPS at overview with 10M anchors
- ≥30 FPS at deep zoom on dense regions
- GPU-accelerated verification with CPU fallback
*/

use anyhow::Result;
use std::sync::Arc;

#[cfg(feature = "webgpu")]
pub mod pipeline;
#[cfg(feature = "webgpu")]
pub mod render;
#[cfg(feature = "webgpu")]
pub mod shaders;
pub mod lod;
#[cfg(feature = "webgpu")]
pub mod tiling;
pub mod vector_export;

pub use lod::*;
#[cfg(feature = "webgpu")]
pub use tiling::*;
#[cfg(feature = "webgpu")]
pub use render::*;
#[cfg(feature = "webgpu")]
pub use pipeline::*;

/// Main GPU rendering context
#[cfg(feature = "webgpu")]
pub struct GpuRenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface_config: wgpu::SurfaceConfiguration,
    lod_manager: LodManager,
    tile_manager: TileManager,
    render_pipeline: Arc<RenderPipeline>,
}

#[cfg(feature = "webgpu")]
impl GpuRenderer {
    /// Create a new GPU renderer with the specified configuration
    pub async fn new(window: &winit::window::Window) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(window) }?;
        
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to request GPU adapter"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::PUSH_CONSTANTS
                        | wgpu::Features::MULTI_DRAW_INDIRECT
                        | wgpu::Features::INDIRECT_FIRST_INSTANCE,
                    limits: wgpu::Limits {
                        max_push_constant_size: 128,
                        ..Default::default()
                    },
                    label: Some("DotX GPU Device"),
                },
                None,
            )
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &surface_config);

        let lod_manager = LodManager::new(device.clone());
        let tile_manager = TileManager::new();
        let render_pipeline = Arc::new(RenderPipeline::new(device.clone(), queue.clone(), &surface_config).await?);

        Ok(Self {
            device,
            queue,
            surface_config,
            lod_manager,
            tile_manager,
            render_pipeline,
        })
    }

    /// Render a frame with the current view parameters
    pub fn render_frame(
        &mut self,
        surface: &wgpu::Surface,
        anchors: &[dotx_core::types::Anchor],
        viewport: &Viewport,
    ) -> Result<()> {
        let output = surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("DotX Render Encoder"),
        });

        // Determine LOD level based on zoom
        let lod_level = self.lod_manager.determine_lod_level(viewport);
        
        // Update tiles for current viewport
        self.tile_manager.update_tiles(viewport, anchors)?;
        
        // Render using appropriate pipeline
        match lod_level {
            LodLevel::Overview => {
                self.render_pipeline.render_overview(&mut encoder, &view, viewport)?;
            }
            LodLevel::MidZoom => {
                self.render_pipeline.render_mid_zoom(&mut encoder, &view, viewport)?;
            }
            LodLevel::DeepZoom => {
                self.render_pipeline.render_deep_zoom(&mut encoder, &view, viewport)?;
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Resize the renderer
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
        }
    }
}

/// Viewport parameters for rendering
#[derive(Debug, Clone)]
pub struct Viewport {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub width: u32,
    pub height: u32,
    pub zoom_level: f32,
}

impl Viewport {
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64, width: u32, height: u32) -> Self {
        let zoom_level = (width as f64 / (x_max - x_min)).log2() as f32;
        Self {
            x_min,
            x_max,
            y_min,
            y_max,
            width,
            height,
            zoom_level,
        }
    }

    pub fn pixel_to_world(&self, pixel_x: f32, pixel_y: f32) -> (f64, f64) {
        let world_x = self.x_min + (pixel_x as f64 / self.width as f64) * (self.x_max - self.x_min);
        let world_y = self.y_min + (pixel_y as f64 / self.height as f64) * (self.y_max - self.y_min);
        (world_x, world_y)
    }

    pub fn world_to_pixel(&self, world_x: f64, world_y: f64) -> (f32, f32) {
        let pixel_x = ((world_x - self.x_min) / (self.x_max - self.x_min)) * self.width as f64;
        let pixel_y = ((world_y - self.y_min) / (self.y_max - self.y_min)) * self.height as f64;
        (pixel_x as f32, pixel_y as f32)
    }
}

// Re-export important types
pub use dotx_core::types::Anchor;

// Shared render style for CLI and GUI parity
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RenderStyle {
    pub show_plus: bool,
    pub show_minus: bool,
    pub flip: String, // one of: none|x|y|xy|rcx|rcy|rcxy
    pub theme: String,
    pub legend: bool,
    pub scale_bar: bool,
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            show_plus: true,
            show_minus: true,
            flip: "none".to_string(),
            theme: "default".to_string(),
            legend: true,
            scale_bar: true,
        }
    }
}
