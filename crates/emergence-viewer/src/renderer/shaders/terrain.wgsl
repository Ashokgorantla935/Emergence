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

    let world = vec2<f32>(
        inst.world_pos.x + vertex.position.x,
        inst.world_pos.y + vertex.position.y,
    );

    var clip = camera.view_proj * vec4<f32>(world, 0.0, 1.0);

    // Integer snap to prevent subpixel gaps between tiles
    let resolution = vec2<f32>(1920.0, 1080.0);
    let snapped_x = floor(clip.x / clip.w * resolution.x * 0.5 + 0.5) / (resolution.x * 0.5) * clip.w;
    let snapped_y = floor(clip.y / clip.w * resolution.y * 0.5 + 0.5) / (resolution.y * 0.5) * clip.w;
    clip.x = snapped_x;
    clip.y = snapped_y;

    out.clip_position = clip;

    // Map the quad's 0-1 UV to the specific tile region in the atlas
    let tile_size = 1.0 / 32.0; // 16px / 512px atlas
    out.atlas_uv = inst.tile_uv + vertex.uv * tile_size;
    out.flags = inst.flags;
    out.world_uv = world / 256.0; // normalized world coords for water effects

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = water_time.time;
    let is_water = in.flags > 0.5;

    var color = textureSample(t_atlas, s_atlas, in.atlas_uv);

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
