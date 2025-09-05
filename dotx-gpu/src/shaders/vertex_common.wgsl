// Common vertex shader structures and functions

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
};

struct InstanceInput {
    @location(2) instance_position: vec2<u32>,  // 16-bit normalized tile coordinates
    @location(3) instance_size: vec2<f32>,      // Size in world units
    @location(4) instance_color: vec4<f32>,     // RGBA color
    @location(5) instance_data: vec4<u32>,      // Custom instance data
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_position: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) lod_factor: f32,
};

struct PushConstants {
    tile_bounds: vec4<f64>,     // x_min, y_min, x_max, y_max in world coordinates
    colormap: u32,              // Color mapping mode
    show_forward: u32,          // Show forward strand
    show_reverse: u32,          // Show reverse strand
    line_width: f32,            // Line width for rendering
};

@group(0) @binding(0) var<uniform> transform: TransformUniforms;
@group(0) @binding(1) var density_texture: texture_2d<f32>;
@group(0) @binding(2) var density_sampler: sampler;

// Push constants for tile-specific data
var<push_constant> push: PushConstants;