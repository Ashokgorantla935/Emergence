// Instanced quad terrain shader — each terrain cell is a textured quad
// sampling a real 16x16 tile from the sprite atlas.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    pixels_per_unit: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var t_atlas: texture_2d<f32>;
@group(1) @binding(1) var s_atlas: sampler;

// group 2: water time + signal tint uniform
struct WaterTime {
    time:           f32,
    signal_danger:  f32,
    signal_comfort: f32,
    signal_grief:   f32,
};
@group(2) @binding(0) var<uniform> water_time: WaterTime;

struct VertexInput {
    @location(0) position: vec2<f32>,  // unit quad corner (0-1)
    @location(1) uv:       vec2<f32>,  // 0-1 within quad
};

struct InstanceInput {
    @location(2) world_pos: vec2<f32>, // world x, y of this cell
    @location(3) tile_uv:   vec2<f32>, // UV origin in the atlas for this tile
    @location(4) flags:     f32,       // 1.0 = water, 0.0 = land
    @location(5) _pad:      f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) atlas_uv:  vec2<f32>,
    @location(1) flags:     f32,
    @location(2) world_uv:  vec2<f32>, // for water animation
};

@vertex
fn vs_main(vertex: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // Integer grid positions — no floating-point drift
    let world = vec2<f32>(
        inst.world_pos.x + vertex.position.x,
        inst.world_pos.y + vertex.position.y,
    );

    out.clip_position = camera.view_proj * vec4<f32>(world, 0.0, 1.0);

    // Map the quad's 0-1 UV to the specific tile region in the atlas
    // Apply 0.5px inset (0.01 in UV space for 16px tile in 512px atlas) to prevent bleeding
    let tile_size = 1.0 / 32.0; // 16px / 512px atlas
    let inset = 0.001; // ~0.5px inset at 512px atlas
    let uv_min = inst.tile_uv + vec2<f32>(inset, inset);
    let uv_range = tile_size - 2.0 * inset;
    out.atlas_uv = uv_min + vertex.uv * uv_range;
    out.flags = inst.flags;
    out.world_uv = world / 256.0; // normalized world coords for water effects

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = water_time.time;
    let is_water = (u32(in.flags + 0.5) == 1u);

    // Solid WorldBox palette base color per biome (flags: 0=grass, 1=water, 2=forest, 3=desert, 4=mountain, 5=wetland)
    let biome_id = u32(in.flags + 0.5);
    var base: vec3<f32>;
    switch biome_id {
        case 0u: { base = vec3<f32>(0.667, 0.741, 0.239); } // #aabd3d grassland
        case 1u: { base = vec3<f32>(0.0, 0.471, 0.945); }   // #0078f1 water
        case 2u: { base = vec3<f32>(0.314, 0.471, 0.020); } // #507805 forest
        case 3u: { base = vec3<f32>(0.973, 0.847, 0.471); } // #f8d878 desert
        case 4u: { base = vec3<f32>(0.439, 0.329, 0.231); } // #70543b mountain
        case 5u: { base = vec3<f32>(0.404, 0.545, 0.0); }   // #678b00 wetland
        default: { base = vec3<f32>(0.667, 0.741, 0.239); }
    }

    // Add subtle per-cell noise variation (±5% brightness from world position)
    let noise = fract(sin(dot(in.world_uv * 256.0, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let brightness = 0.95 + noise * 0.10; // 0.95 to 1.05
    var color = vec4<f32>(base * brightness, 1.0);

    // Overlay atlas tile on top — only where tile pixels are fully opaque
    let tile_color = textureSample(t_atlas, s_atlas, in.atlas_uv);
    if (tile_color.a > 0.9) {
        color = vec4<f32>(mix(color.rgb, tile_color.rgb, 0.7), 1.0); // blend 70% tile, 30% base
    }

    if is_water {
        // Subtle animated UV distortion for water shimmer
        let tile_size = 1.0 / 32.0;
        let local_uv = fract((in.atlas_uv) / tile_size); // 0-1 within tile
        let wave_offset = vec2<f32>(
            sin(in.world_uv.y * 80.0 + t * 2.0) * 0.02,
            sin(in.world_uv.x * 80.0 + t * 1.7) * 0.02,
        );
        // Re-sample with wave offset, clamped within the tile
        let tile_origin = in.atlas_uv - local_uv * tile_size;
        let new_local = clamp(local_uv + wave_offset, vec2<f32>(0.01), vec2<f32>(0.99));
        let wave_uv = tile_origin + new_local * tile_size;
        color = textureSample(t_atlas, s_atlas, wave_uv);

        // Brightness pulse
        let pulse = 1.0 + sin(t * 2.0) * 0.05;
        color = vec4<f32>(color.rgb * pulse, color.a);
    } else {
        // Signal tinting for land
        let d = clamp(water_time.signal_danger,  0.0, 1.0);
        let c = clamp(water_time.signal_comfort, 0.0, 1.0);
        let g = clamp(water_time.signal_grief,   0.0, 1.0);

        let r_mult = 1.0 + d * 0.10 - g * 0.10;
        let g_mult = 1.0 - d * 0.10 - g * 0.10;
        let b_mult = 1.0 - d * 0.10 + c * (-0.05) + g * 0.10;

        color = vec4<f32>(
            clamp(color.r * r_mult, 0.0, 1.0),
            clamp(color.g * g_mult, 0.0, 1.0),
            clamp(color.b * b_mult, 0.0, 1.0),
            color.a,
        );
    }

    return color;
}
