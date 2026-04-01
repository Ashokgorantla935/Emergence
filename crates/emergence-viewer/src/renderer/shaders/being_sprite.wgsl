// Sprite shader for being instances.
// Renders instanced billboard quads sampling the 512x512 pixel-art atlas.
// Enforces 8px minimum screen size so beings are never invisible dots.

struct CameraUniform {
    view_proj:            mat4x4<f32>,
    pixels_per_unit:      f32,
    _pad0:                f32,
    _pad1:                f32,
    _pad2:                f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

@group(1) @binding(0) var sprite_atlas:  texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

// ── Vertex inputs ────────────────────────────────────────────────────────────

struct VertexInput {
    @location(0) vertex_pos: vec2<f32>,   // unit quad [-0.5, 0.5]
};

struct InstanceInput {
    @location(1) world_pos:    vec2<f32>,  // world-space centre
    @location(2) atlas_uv:     vec2<f32>,  // atlas UV top-left
    @location(3) atlas_size:   vec2<f32>,  // UV extent (1/32, 1/32)
    @location(4) emotion_tint: vec3<f32>,  // clothing tint
    @location(5) skin_tone:    vec3<f32>,  // skin replacement colour
    @location(6) size:         f32,        // world-unit size
    @location(7) brightness:   f32,        // multiplier (1.5 when urgent)
    @location(8) alpha:        f32,        // opacity
    @location(9) _pad:         f32,        // alignment pad
};

// ── Vertex output ────────────────────────────────────────────────────────────

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv:           vec2<f32>,
    @location(1) emotion_tint: vec3<f32>,
    @location(2) skin_tone:    vec3<f32>,
    @location(3) brightness:   f32,
    @location(4) alpha:        f32,
};

// ── Vertex shader ────────────────────────────────────────────────────────────

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // FIX 6: 8px minimum — smaller beings feel more numerous (WorldBox aesthetic)
    let screen_size = max(instance.size * camera.pixels_per_unit, 8.0);
    let final_size  = screen_size / camera.pixels_per_unit;

    // Billboard quad centred on world_pos.
    let world_xy = instance.world_pos + vertex.vertex_pos * final_size;
    out.clip_position = camera.view_proj * vec4<f32>(world_xy, 0.0, 1.0);

    // Map vertex [-0.5,0.5] to atlas UV within the cell.
    out.uv           = instance.atlas_uv + (vertex.vertex_pos + vec2<f32>(0.5, 0.5)) * instance.atlas_size;
    out.emotion_tint = instance.emotion_tint;
    out.skin_tone    = instance.skin_tone;
    out.brightness   = instance.brightness;
    out.alpha        = instance.alpha;
    return out;
}

// ── Fragment shader ──────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_color = textureSample(sprite_atlas, atlas_sampler, in.uv);

    // Discard transparent pixels (alpha threshold 0.1 = nearly transparent).
    if atlas_color.a < 0.1 {
        discard;
    }

    // FIX 3: two-tone sprite — atlas encodes skin pixels as near-white (r>0.7),
    // cloth pixels as mid-gray (r~0.5). Threshold selects skin_tone vs emotion_tint.
    let is_skin = atlas_color.r > 0.7;
    let pixel_color = select(in.emotion_tint, in.skin_tone, is_skin);
    let final_rgb = pixel_color * in.brightness;
    return vec4<f32>(final_rgb, atlas_color.a * in.alpha);
}
