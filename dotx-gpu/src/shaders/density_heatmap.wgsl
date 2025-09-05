// Density heatmap shader for overview level rendering
// Renders precomputed density textures as colored heatmaps

// Import common utilities
// Note: In actual implementation, these would be included via shader preprocessing

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    
    // Transform vertex position to world coordinates
    let world_pos = vec2<f64>(
        push.tile_bounds.x + f64(vertex.position.x) * (push.tile_bounds.z - push.tile_bounds.x),
        push.tile_bounds.y + f64(vertex.position.y) * (push.tile_bounds.w - push.tile_bounds.y)
    );
    
    // Convert to clip space using high-precision transform
    output.clip_position = transform_instance_position(
        vec2<u32>(u32(vertex.position.x * 65535.0), u32(vertex.position.y * 65535.0)),
        push.tile_bounds,
        transform
    );
    
    output.tex_coords = vertex.tex_coords;
    output.world_position = vec2<f32>(world_pos);
    output.color = vec4<f32>(1.0);
    output.lod_factor = calculate_lod_factor(world_pos, transform);
    
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample density from precomputed texture
    let density = textureSample(density_texture, density_sampler, input.tex_coords).r;
    
    // Skip completely transparent pixels
    if (density < 0.001) {
        discard;
    }
    
    // Convert density to color using selected colormap
    var color = density_to_color(density, push.colormap);
    
    // Apply LOD-based alpha modulation
    color.a *= input.lod_factor;
    
    // Apply gamma correction for better visual appearance
    color.r = pow(color.r, 1.0 / 2.2);
    color.g = pow(color.g, 1.0 / 2.2);
    color.b = pow(color.b, 1.0 / 2.2);
    
    return color;
}

// Compute shader for density texture generation (optional, can be done CPU-side)
@compute @workgroup_size(8, 8)
fn cs_generate_density(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let tex_size = textureDimensions(density_texture);
    if (global_id.x >= tex_size.x || global_id.y >= tex_size.y) {
        return;
    }
    
    // This would be implemented with anchor data buffer binding
    // For now, this is a placeholder showing the structure
    let pixel_coord = vec2<u32>(global_id.x, global_id.y);
    let normalized_coord = vec2<f32>(
        f32(pixel_coord.x) / f32(tex_size.x),
        f32(pixel_coord.y) / f32(tex_size.y)
    );
    
    // Convert to world coordinates
    let world_coord = vec2<f64>(
        push.tile_bounds.x + f64(normalized_coord.x) * (push.tile_bounds.z - push.tile_bounds.x),
        push.tile_bounds.y + f64(normalized_coord.y) * (push.tile_bounds.w - push.tile_bounds.y)
    );
    
    // Count nearby anchors (this would access an anchor buffer in real implementation)
    let density = 0.0; // Placeholder
    
    textureStore(density_texture, pixel_coord, vec4<f32>(density, 0.0, 0.0, 1.0));
}