// Transform utilities for high-precision coordinate handling
// Uses 16-bit normalized coordinates within tiles with high-precision transforms

struct TransformUniforms {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    tile_offset: vec2<f64>,      // High-precision tile world offset
    tile_scale: vec2<f64>,       // High-precision scale factor
    viewport_size: vec2<f32>,    // Screen viewport dimensions
    zoom_level: f32,             // Current zoom level
    _padding: f32,
};

// Convert 16-bit normalized tile coordinates to world coordinates
fn tile_to_world(tile_pos: vec2<u32>, tile_bounds: vec4<f64>) -> vec2<f64> {
    let normalized = vec2<f64>(
        f64(tile_pos.x) / 65535.0,
        f64(tile_pos.y) / 65535.0
    );
    
    return vec2<f64>(
        tile_bounds.x + normalized.x * (tile_bounds.z - tile_bounds.x),
        tile_bounds.y + normalized.y * (tile_bounds.w - tile_bounds.y)
    );
}

// Convert world coordinates to screen coordinates using high-precision transform
fn world_to_screen(world_pos: vec2<f64>, transform: TransformUniforms) -> vec2<f32> {
    // Apply tile offset and scale with high precision
    let relative_pos = (world_pos - transform.tile_offset) * transform.tile_scale;
    
    // Convert to clip space
    let clip_pos = vec4<f32>(
        f32(relative_pos.x),
        f32(relative_pos.y),
        0.0,
        1.0
    );
    
    // Apply projection matrix
    let projected = transform.projection_matrix * transform.view_matrix * clip_pos;
    
    // Convert to screen coordinates
    return vec2<f32>(
        (projected.x / projected.w + 1.0) * 0.5 * transform.viewport_size.x,
        (1.0 - projected.y / projected.w) * 0.5 * transform.viewport_size.y
    );
}

// Optimized transform for instanced rendering
fn transform_instance_position(
    tile_pos: vec2<u32>,
    tile_bounds: vec4<f64>,
    transform: TransformUniforms
) -> vec4<f32> {
    let world_pos = tile_to_world(tile_pos, tile_bounds);
    let relative_pos = (world_pos - transform.tile_offset) * transform.tile_scale;
    
    return transform.projection_matrix * transform.view_matrix * vec4<f32>(
        f32(relative_pos.x),
        f32(relative_pos.y),
        0.0,
        1.0
    );
}

// Calculate Level-of-Detail factor based on zoom and distance
fn calculate_lod_factor(world_pos: vec2<f64>, transform: TransformUniforms) -> f32 {
    let distance_from_center = length(world_pos - transform.tile_offset);
    let lod_factor = transform.zoom_level / (1.0 + f32(distance_from_center) * 0.001);
    return clamp(lod_factor, 0.0, 1.0);
}

// Color encoding utilities for density visualization
fn density_to_color(density: f32, colormap: u32) -> vec4<f32> {
    switch colormap {
        case 0u: {
            // Heat colormap (blue -> red)
            let r = smoothstep(0.3, 1.0, density);
            let g = smoothstep(0.1, 0.7, density) * (1.0 - smoothstep(0.7, 1.0, density));
            let b = 1.0 - smoothstep(0.0, 0.5, density);
            return vec4<f32>(r, g, b, density);
        }
        case 1u: {
            // Plasma colormap approximation
            let r = smoothstep(0.0, 1.0, density);
            let g = smoothstep(0.2, 0.8, density) * smoothstep(0.8, 0.2, density);
            let b = 1.0 - smoothstep(0.3, 1.0, density);
            return vec4<f32>(r * 0.9, g * 0.7 + 0.1, b * 0.8 + 0.2, density);
        }
        default: {
            // Grayscale
            return vec4<f32>(density, density, density, density);
        }
    }
}

// Strand color encoding
fn strand_to_color(strand: i32) -> vec4<f32> {
    switch strand {
        case 1: {
            // Forward strand - blue
            return vec4<f32>(0.16, 0.44, 0.94, 1.0);
        }
        case -1: {
            // Reverse strand - red
            return vec4<f32>(0.90, 0.22, 0.21, 1.0);
        }
        default: {
            // Unknown strand - gray
            return vec4<f32>(0.5, 0.5, 0.5, 1.0);
        }
    }
}

// Anti-aliasing helper for smooth edges
fn compute_coverage(pixel_center: vec2<f32>, line_start: vec2<f32>, line_end: vec2<f32>, width: f32) -> f32 {
    let line_vec = line_end - line_start;
    let line_length = length(line_vec);
    
    if (line_length < 0.001) {
        return 0.0;
    }
    
    let line_dir = line_vec / line_length;
    let perp = vec2<f32>(-line_dir.y, line_dir.x);
    
    let to_pixel = pixel_center - line_start;
    let along_line = dot(to_pixel, line_dir);
    let across_line = abs(dot(to_pixel, perp));
    
    // Check if pixel is within line segment bounds
    if (along_line < 0.0 || along_line > line_length) {
        return 0.0;
    }
    
    // Compute coverage based on distance from line
    let distance_from_line = across_line;
    let half_width = width * 0.5;
    
    return 1.0 - smoothstep(half_width - 0.5, half_width + 0.5, distance_from_line);
}