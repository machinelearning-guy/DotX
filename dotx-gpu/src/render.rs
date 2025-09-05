/*!
# GPU Rendering Pipeline Implementation

Implements the three-tier rendering system with WebGPU/wgpu for high-performance
visualization of genomic anchors with LOD support.
*/

use std::sync::Arc;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use crate::{Viewport, LodLevel};

/// Transform uniforms for high-precision coordinate handling
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct TransformUniforms {
    pub view_matrix: [[f32; 4]; 4],
    pub projection_matrix: [[f32; 4]; 4],
    pub tile_offset: [f64; 2],      // High-precision tile world offset
    pub tile_scale: [f64; 2],       // High-precision scale factor  
    pub viewport_size: [f32; 2],    // Screen viewport dimensions
    pub zoom_level: f32,            // Current zoom level
    pub _padding: f32,
}

/// Push constants for tile-specific rendering parameters
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct PushConstants {
    pub tile_bounds: [f64; 4],      // x_min, y_min, x_max, y_max
    pub colormap: u32,              // Color mapping mode
    pub show_forward: u32,          // Show forward strand
    pub show_reverse: u32,          // Show reverse strand
    pub line_width: f32,            // Line width for rendering
    pub _padding: [u32; 3],
}

/// Vertex data for basic rendering
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
}

/// Instance data for point rendering
#[repr(C)] 
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct PointInstance {
    pub position: [u32; 2],         // 16-bit normalized tile coordinates
    pub target_range: [u32; 2],     // Target start/end
    pub query_range: [u32; 2],      // Query start/end
    pub strand: i32,                // Strand (+1/-1)
    pub identity: f32,              // Sequence identity
    pub mapq: u32,                  // Mapping quality
    pub anchor_id: u32,             // Unique identifier
}

/// Main rendering pipeline
pub struct RenderPipeline {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    
    // Shader modules
    overview_shader: wgpu::ShaderModule,
    polyline_shader: wgpu::ShaderModule,
    points_shader: wgpu::ShaderModule,
    
    // Render pipelines
    overview_pipeline: wgpu::RenderPipeline,
    polyline_pipeline: wgpu::RenderPipeline,
    points_pipeline: wgpu::RenderPipeline,
    
    // Bind group layouts
    transform_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    
    // Uniform buffers
    transform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
    
    // Vertex buffers for fullscreen quad
    quad_vertex_buffer: wgpu::Buffer,
    quad_index_buffer: wgpu::Buffer,
}

impl RenderPipeline {
    pub async fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Result<Self> {
        // Create shader modules
        let overview_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Overview Shader"),
            source: wgpu::ShaderSource::Wgsl(
                format!("{}\n{}\n{}", 
                    crate::shaders::TRANSFORM_UTILS,
                    crate::shaders::VERTEX_COMMON,
                    crate::shaders::DENSITY_HEATMAP_SHADER
                ).into()
            ),
        });

        let polyline_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Polyline Shader"),
            source: wgpu::ShaderSource::Wgsl(
                format!("{}\n{}\n{}", 
                    crate::shaders::TRANSFORM_UTILS,
                    crate::shaders::VERTEX_COMMON,
                    crate::shaders::POLYLINE_SHADER
                ).into()
            ),
        });

        let points_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Points Shader"),
            source: wgpu::ShaderSource::Wgsl(
                format!("{}\n{}\n{}", 
                    crate::shaders::TRANSFORM_UTILS,
                    crate::shaders::VERTEX_COMMON,
                    crate::shaders::INSTANCED_POINTS_SHADER
                ).into()
            ),
        });

        // Create bind group layouts
        let transform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Transform Bind Group Layout"),
        });

        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Texture Bind Group Layout"),
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&transform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                range: 0..std::mem::size_of::<PushConstants>() as u32,
            }],
        });

        // Create render pipelines
        let overview_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Overview Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &overview_shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &overview_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let polyline_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Polyline Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &polyline_shader,
                entry_point: "vs_quad_main",
                buffers: &[Vertex::desc()], // Will be expanded for line rendering
            },
            fragment: Some(wgpu::FragmentState {
                module: &polyline_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let points_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Points Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &points_shader,
                entry_point: "vs_main",
                buffers: &[PointInstance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &points_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Create transform uniform buffer
        let transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Transform Buffer"),
            size: std::mem::size_of::<TransformUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &transform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: transform_buffer.as_entire_binding(),
                },
            ],
            label: Some("Transform Bind Group"),
        });

        // Create fullscreen quad buffers
        let quad_vertices = &[
            Vertex { position: [-1.0, -1.0], tex_coords: [0.0, 1.0] },
            Vertex { position: [ 1.0, -1.0], tex_coords: [1.0, 1.0] },
            Vertex { position: [ 1.0,  1.0], tex_coords: [1.0, 0.0] },
            Vertex { position: [-1.0,  1.0], tex_coords: [0.0, 0.0] },
        ];

        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(quad_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let quad_indices: &[u16] = &[0, 1, 2, 0, 2, 3];
        let quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Index Buffer"),
            contents: bytemuck::cast_slice(quad_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Self {
            device,
            queue,
            overview_shader,
            polyline_shader,
            points_shader,
            overview_pipeline,
            polyline_pipeline,
            points_pipeline,
            transform_bind_group_layout,
            texture_bind_group_layout,
            transform_buffer,
            transform_bind_group,
            quad_vertex_buffer,
            quad_index_buffer,
        })
    }

    /// Render overview level (density heatmap)
    pub fn render_overview(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        viewport: &Viewport,
    ) -> Result<()> {
        // Update transform uniforms
        self.update_transform_uniforms(viewport)?;

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Overview Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&self.overview_pipeline);
        render_pass.set_bind_group(0, &self.transform_bind_group, &[]);
        
        // Set push constants
        let push_constants = PushConstants {
            tile_bounds: [viewport.x_min, viewport.y_min, viewport.x_max, viewport.y_max],
            colormap: 0, // Heat colormap
            show_forward: 1,
            show_reverse: 1,
            line_width: 1.0,
            _padding: [0; 3],
        };
        render_pass.set_push_constants(wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(&push_constants));

        render_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.quad_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..1);

        Ok(())
    }

    /// Render mid-zoom level (polylines)
    pub fn render_mid_zoom(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        viewport: &Viewport,
    ) -> Result<()> {
        // Update transform uniforms
        self.update_transform_uniforms(viewport)?;

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Mid Zoom Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&self.polyline_pipeline);
        render_pass.set_bind_group(0, &self.transform_bind_group, &[]);
        
        // Set push constants for polyline rendering
        let push_constants = PushConstants {
            tile_bounds: [viewport.x_min, viewport.y_min, viewport.x_max, viewport.y_max],
            colormap: 0,
            show_forward: 1,
            show_reverse: 1,
            line_width: 2.0,
            _padding: [0; 3],
        };
        render_pass.set_push_constants(wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(&push_constants));

        // TODO: Render actual polyline data
        // This would involve loading chain segments and rendering them as instanced quads

        Ok(())
    }

    /// Render deep-zoom level (instanced points)
    pub fn render_deep_zoom(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        viewport: &Viewport,
    ) -> Result<()> {
        // Update transform uniforms
        self.update_transform_uniforms(viewport)?;

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Deep Zoom Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&self.points_pipeline);
        render_pass.set_bind_group(0, &self.transform_bind_group, &[]);
        
        // Set push constants for point rendering
        let push_constants = PushConstants {
            tile_bounds: [viewport.x_min, viewport.y_min, viewport.x_max, viewport.y_max],
            colormap: 0,
            show_forward: 1,
            show_reverse: 1,
            line_width: 1.0,
            _padding: [0; 3],
        };
        render_pass.set_push_constants(wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(&push_constants));

        // TODO: Render actual point instances
        // This would involve loading anchor data as instances and drawing them

        Ok(())
    }

    /// Update transform uniforms for current viewport
    fn update_transform_uniforms(&self, viewport: &Viewport) -> Result<()> {
        let uniforms = TransformUniforms {
            view_matrix: glam::Mat4::IDENTITY.to_cols_array_2d(),
            projection_matrix: glam::Mat4::orthographic_rh(
                0.0, viewport.width as f32,
                0.0, viewport.height as f32,
                -1.0, 1.0
            ).to_cols_array_2d(),
            tile_offset: [viewport.x_min, viewport.y_min],
            tile_scale: [
                (viewport.x_max - viewport.x_min) / viewport.width as f64,
                (viewport.y_max - viewport.y_min) / viewport.height as f64
            ],
            viewport_size: [viewport.width as f32, viewport.height as f32],
            zoom_level: viewport.zoom_level,
            _padding: 0.0,
        };

        self.queue.write_buffer(&self.transform_buffer, 0, bytemuck::bytes_of(&uniforms));
        Ok(())
    }
}

// Vertex buffer layout implementations
impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

impl PointInstance {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PointInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Uint32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[u32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Uint32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Uint32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[u32; 6]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Sint32,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[u32; 6]>() as wgpu::BufferAddress + std::mem::size_of::<i32>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[u32; 6]>() as wgpu::BufferAddress + std::mem::size_of::<i32>() as wgpu::BufferAddress + std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Uint32,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[u32; 6]>() as wgpu::BufferAddress + std::mem::size_of::<i32>() as wgpu::BufferAddress + std::mem::size_of::<f32>() as wgpu::BufferAddress + std::mem::size_of::<u32>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}