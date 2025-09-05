/*!
# WebGPU Shader Collection

Contains all WGSL shaders for the DotX rendering pipeline, including:
- Overview density heatmap rendering
- Mid-zoom polyline segment rendering  
- Deep-zoom instanced point rendering
- High-precision coordinate transformations
*/

/// Overview density heatmap shader
pub const DENSITY_HEATMAP_SHADER: &str = include_str!("density_heatmap.wgsl");

/// Mid-zoom polyline rendering shader  
pub const POLYLINE_SHADER: &str = include_str!("polyline.wgsl");

/// Deep-zoom instanced point rendering shader
pub const INSTANCED_POINTS_SHADER: &str = include_str!("instanced_points.wgsl");

/// High-precision coordinate transformation utilities
pub const TRANSFORM_UTILS: &str = include_str!("transform_utils.wgsl");

/// Common vertex shader functions
pub const VERTEX_COMMON: &str = include_str!("vertex_common.wgsl");