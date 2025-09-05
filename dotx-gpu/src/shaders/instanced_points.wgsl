// Instanced points shader for deep-zoom level rendering
// Efficiently renders millions of individual anchors as points or small quads

struct PointInstance {
    @location(0) position: vec2<u32>,      // 16-bit normalized tile coordinates
    @location(1) target_range: vec2<u32>,  // Target start/end coordinates
    @location(2) query_range: vec2<u32>,   // Query start/end coordinates  
    @location(3) strand: i32,              // Strand (+1/-1)
    @location(4) identity: f32,            // Sequence identity (0-1)
    @location(5) mapq: u32,                // Mapping quality
    @location(6) anchor_id: u32,           // Unique anchor identifier
};

struct PointVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) point_size: f32,
    @location(3) anchor_data: vec4<f32>,   // identity, mapq, etc.
    @location(4) quad_coord: vec2<f32>,    // For anti-aliasing
};

// Vertex shader for instanced point rendering
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: PointInstance
) -> PointVertexOutput {
    var output: PointVertexOutput;
    
    // Generate quad vertices for each point instance
    let quad_positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),  // Bottom-left
        vec2<f32>( 1.0, -1.0),  // Bottom-right
        vec2<f32>(-1.0,  1.0),  // Top-left
        vec2<f32>( 1.0,  1.0)   // Top-right
    );
    let quad_pos = quad_positions[vertex_index % 4u];
    
    // Convert instance position to world coordinates
    let world_pos = tile_to_world(instance.position, push.tile_bounds);
    
    // Calculate point size based on zoom and anchor properties
    let base_size = 2.0;
    let zoom_factor = clamp(transform.zoom_level / 16.0, 0.5, 4.0);
    let quality_factor = 1.0 + f32(instance.mapq) / 255.0 * 0.5;
    let point_size = base_size * zoom_factor * quality_factor;
    
    // Expand quad based on point size
    let pixel_size = point_size / transform.viewport_size.y; // Size in clip space
    let expanded_world = world_pos + vec2<f64>(quad_pos) * f64(pixel_size);
    
    // Transform to clip space
    let relative_pos = (expanded_world - transform.tile_offset) * transform.tile_scale;
    output.clip_position = transform.projection_matrix * transform.view_matrix * vec4<f32>(
        f32(relative_pos.x),
        f32(relative_pos.y),
        0.0,
        1.0
    );
    
    output.world_position = vec2<f32>(world_pos);
    output.point_size = point_size;
    output.quad_coord = quad_pos;
    
    // Color based on strand and identity
    var base_color = strand_to_color(instance.strand);
    
    // Modulate color by sequence identity
    let identity_factor = clamp(instance.identity, 0.0, 1.0);
    base_color.rgb = mix(vec3<f32>(0.5, 0.5, 0.5), base_color.rgb, identity_factor);
    
    // Add quality-based brightness
    let quality_brightness = 0.8 + 0.2 * (f32(instance.mapq) / 255.0);
    base_color.rgb *= quality_brightness;
    
    output.color = base_color;
    output.anchor_data = vec4<f32>(
        instance.identity,
        f32(instance.mapq) / 255.0,
        f32(instance.target_range.y - instance.target_range.x),
        f32(instance.query_range.y - instance.query_range.x)
    );
    
    return output;
}

@fragment
fn fs_main(input: PointVertexOutput) -> @location(0) vec4<f32> {
    var color = input.color;
    
    // Apply strand filtering
    let is_forward = color.b < 0.5; // Forward strand is more blue
    if (is_forward && push.show_forward == 0u) {
        discard;
    }
    if (!is_forward && push.show_reverse == 0u) {
        discard;
    }
    
    // Create circular points with anti-aliasing
    let distance_from_center = length(input.quad_coord);
    let radius = 1.0;
    let edge_softness = 2.0 / input.point_size; // Adaptive edge softness
    
    // Smooth circle with anti-aliasing
    let alpha = 1.0 - smoothstep(radius - edge_softness, radius + edge_softness, distance_from_center);
    color.a *= alpha;
    
    // Add highlight for high-quality anchors
    if (input.anchor_data.y > 0.9) { // High MAPQ
        let highlight = smoothstep(0.7, 1.0, 1.0 - distance_from_center);
        color.rgb = mix(color.rgb, vec3<f32>(1.0), highlight * 0.3);
    }
    
    // Add subtle size-based alpha modulation
    let size_alpha = clamp(input.point_size / 4.0, 0.3, 1.0);
    color.a *= size_alpha;
    
    return color;
}

// Alternative shader for rectangular anchor visualization
@fragment
fn fs_rect_main(input: PointVertexOutput) -> @location(0) vec4<f32> {
    var color = input.color;
    
    // Create rectangular anchors with aspect ratio based on anchor length
    let target_length = input.anchor_data.z;
    let query_length = input.anchor_data.w;
    let aspect_ratio = target_length / max(query_length, 1.0);
    
    // Adjust quad coordinates based on aspect ratio
    let adjusted_coord = vec2<f32>(
        input.quad_coord.x * clamp(aspect_ratio, 0.1, 10.0),
        input.quad_coord.y
    );
    
    // Rectangular shape with rounded corners
    let corner_radius = 0.1;
    let rect_distance = max(
        abs(adjusted_coord.x) - (1.0 - corner_radius),
        abs(adjusted_coord.y) - (1.0 - corner_radius)
    );
    
    let alpha = 1.0 - smoothstep(
        corner_radius - 2.0 / input.point_size,
        corner_radius + 2.0 / input.point_size,
        rect_distance
    );
    
    color.a *= alpha;
    return color;
}

// Compute shader for point culling and LOD (optional optimization)
struct CullingData {
    visible_count: atomic<u32>,
    visible_indices: array<u32>,
};

@group(1) @binding(0) var<storage, read_write> culling_data: CullingData;
@group(1) @binding(1) var<storage, read> all_instances: array<PointInstance>;

@compute @workgroup_size(64)
fn cs_cull_points(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let instance_index = global_id.x;
    if (instance_index >= arrayLength(&all_instances)) {
        return;
    }
    
    let instance = all_instances[instance_index];
    let world_pos = tile_to_world(instance.position, push.tile_bounds);
    
    // Check if point is within viewport bounds (with margin)
    let margin = 10.0; // Pixel margin
    let viewport_bounds = vec4<f64>(
        transform.tile_offset.x - f64(margin),
        transform.tile_offset.y - f64(margin),
        transform.tile_offset.x + f64(transform.viewport_size.x) + f64(margin),
        transform.tile_offset.y + f64(transform.viewport_size.y) + f64(margin)
    );
    
    if (world_pos.x >= viewport_bounds.x && world_pos.x <= viewport_bounds.z &&
        world_pos.y >= viewport_bounds.y && world_pos.y <= viewport_bounds.w) {
        
        let visible_index = atomicAdd(&culling_data.visible_count, 1u);
        culling_data.visible_indices[visible_index] = instance_index;
    }
}