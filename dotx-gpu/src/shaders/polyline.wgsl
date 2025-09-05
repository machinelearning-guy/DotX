// Polyline shader for mid-zoom level rendering
// Renders chain segments as anti-aliased lines with strand coloring

struct LineVertex {
    @location(0) position: vec2<u32>,  // 16-bit normalized coordinates
    @location(1) next_position: vec2<u32>,  // Next vertex for line direction
    @location(2) strand: i32,          // Strand information (+1/-1)
    @location(3) chain_id: u32,        // Chain identifier
};

struct LineVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) line_direction: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) line_width: f32,
    @location(4) distance_along_line: f32,
};

@vertex
fn vs_main(vertex: LineVertex, @builtin(vertex_index) vertex_index: u32) -> LineVertexOutput {
    var output: LineVertexOutput;
    
    // Convert 16-bit normalized coordinates to world coordinates
    let current_world = tile_to_world(vertex.position, push.tile_bounds);
    let next_world = tile_to_world(vertex.next_position, push.tile_bounds);
    
    // Calculate line direction and perpendicular
    let line_dir = normalize(next_world - current_world);
    let line_perp = vec2<f64>(-line_dir.y, line_dir.x);
    
    // Expand vertex to create line width
    let half_width = f64(push.line_width) * 0.5;
    let offset_factor = select(-1.0, 1.0, (vertex_index % 2u) == 0u);
    let expanded_pos = current_world + line_perp * half_width * offset_factor;
    
    // Transform to clip space
    let relative_pos = (expanded_pos - transform.tile_offset) * transform.tile_scale;
    output.clip_position = transform.projection_matrix * transform.view_matrix * vec4<f32>(
        f32(relative_pos.x),
        f32(relative_pos.y),
        0.0,
        1.0
    );
    
    output.world_position = vec2<f32>(expanded_pos);
    output.line_direction = vec2<f32>(line_dir);
    output.color = strand_to_color(vertex.strand);
    output.line_width = push.line_width;
    output.distance_along_line = f32(length(expanded_pos - current_world));
    
    return output;
}

@fragment
fn fs_main(input: LineVertexOutput) -> @location(0) vec4<f32> {
    var color = input.color;
    
    // Apply strand filtering
    if (input.color.r > 0.7 && push.show_forward == 0u) {
        discard; // Skip forward strand if hidden
    }
    if (input.color.r > 0.7 && push.show_reverse == 0u) {
        discard; // Skip reverse strand if hidden
    }
    
    // Calculate anti-aliasing based on distance from line center
    let distance_from_center = abs(input.distance_along_line);
    let half_width = input.line_width * 0.5;
    let coverage = 1.0 - smoothstep(half_width - 0.5, half_width + 0.5, distance_from_center);
    
    // Apply coverage to alpha
    color.a *= coverage;
    
    // Fade out very thin lines to avoid aliasing
    if (input.line_width < 1.0) {
        color.a *= input.line_width;
    }
    
    // Add subtle highlight for selected chains (could be extended)
    if (input.distance_along_line < 2.0) {
        color.rgb = mix(color.rgb, vec3<f32>(1.0), 0.1);
    }
    
    return color;
}

// Geometry shader alternative using instanced quads
struct QuadVertex {
    @location(0) quad_position: vec2<f32>,  // Local quad vertex (-1 to 1)
};

struct QuadInstance {
    @location(1) line_start: vec2<u32>,     // Line start in 16-bit coords
    @location(2) line_end: vec2<u32>,       // Line end in 16-bit coords
    @location(3) strand: i32,               // Strand information
    @location(4) chain_id: u32,             // Chain identifier
};

@vertex
fn vs_quad_main(vertex: QuadVertex, instance: QuadInstance) -> LineVertexOutput {
    var output: LineVertexOutput;
    
    // Convert to world coordinates
    let start_world = tile_to_world(instance.line_start, push.tile_bounds);
    let end_world = tile_to_world(instance.line_end, push.tile_bounds);
    
    // Calculate line properties
    let line_vec = end_world - start_world;
    let line_length = length(line_vec);
    let line_dir = line_vec / line_length;
    let line_perp = vec2<f64>(-line_dir.y, line_dir.x);
    
    // Position quad vertex
    let along_line = vertex.quad_position.x;
    let across_line = vertex.quad_position.y;
    let world_pos = start_world + 
        line_dir * f64(along_line) * line_length * 0.5 +
        line_perp * f64(across_line) * f64(push.line_width) * 0.5;
    
    // Transform to clip space
    let relative_pos = (world_pos - transform.tile_offset) * transform.tile_scale;
    output.clip_position = transform.projection_matrix * transform.view_matrix * vec4<f32>(
        f32(relative_pos.x),
        f32(relative_pos.y),
        0.0,
        1.0
    );
    
    output.world_position = vec2<f32>(world_pos);
    output.line_direction = vec2<f32>(line_dir);
    output.color = strand_to_color(instance.strand);
    output.line_width = push.line_width;
    output.distance_along_line = across_line * push.line_width * 0.5;
    
    return output;
}