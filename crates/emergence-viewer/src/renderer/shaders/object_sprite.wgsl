// Object sprite shader — used by world objects (resources + structures).
// Simpler variant of being_sprite.wgsl: no skin-tone logic.
// One pipeline handles all objects via the shared atlas.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    pixels_per_unit: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var sprite_atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct ObjectTimeUniform {
    time:       f32,
    delta_time: f32,  // V54: seconds since last sim tick for dead-reckoning
    _pad1:      f32,
    _pad2:      f32,
};
@group(2) @binding(0) var<uniform> obj_time: ObjectTimeUniform;

// Atlas UV row for decorative objects (row 0, tree/bush sprites in 10×10 flora sheet).
// Trees occupy UV y in [0, 1/10). V57: updated from legacy 32×32 atlas.
const TREE_ROW_MIN: f32 = 0.0;
const TREE_ROW_MAX: f32 = 1.0 / 10.0;

struct VertexInput {
    @location(0) vertex_pos: vec2<f32>,
};

struct InstanceInput {
    @location(1) world_pos:         vec2<f32>,
    @location(2) atlas_uv:          vec2<f32>,
    @location(3) atlas_size:        vec2<f32>,
    @location(4) tint:              vec3<f32>,
    @location(5) size:              f32,
    @location(6) alpha:             f32,
    @location(7) velocity:          vec2<f32>,
    @location(8) scale_multiplier:  f32,
    @location(9) _pad_v54:          f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv:          vec2<f32>,
    @location(1) tint:        vec3<f32>,
    @location(2) alpha:       f32,
    @location(3) atlas_uv:    vec2<f32>,   // cell top-left for outline clamping
    @location(4) atlas_size:  vec2<f32>,   // cell size for outline clamping
    @location(5) screen_size: f32,
};

@vertex
fn vs_main(vertex: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // V54: Dead-reckoning — predict position based on velocity and elapsed time
    var world_pos = inst.world_pos + inst.velocity * obj_time.delta_time;

    // V54: Biological scaling — combine base size with type/category multiplier
    let bio_size   = inst.size * inst.scale_multiplier;
    let screen_size = bio_size * camera.pixels_per_unit;

    // V54: LOD discard — cull objects that are sub-pixel at current zoom
    if (screen_size < 1.0) {
        out.clip_position = vec4<f32>(2000.0, 2000.0, 2000.0, 1.0);
        return out;
    }

    // Tree wind sway: only for sprites in atlas row 21 (decor tree/bush).
    // Offset only the top half of the quad (vertex_pos.y > 0) so roots stay planted.
    let in_tree_row = inst.atlas_uv.y >= TREE_ROW_MIN && inst.atlas_uv.y < TREE_ROW_MAX;
    if in_tree_row && vertex.vertex_pos.y > 0.0 {
        let sway = sin(obj_time.time * 1.5 + inst.world_pos.x * 3.0) * 0.03 * bio_size;
        world_pos.x += sway;
    }

    let world       = world_pos + vertex.vertex_pos * bio_size;
    var clip        = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    // Y-sort depth bias: objects further south (higher world Y) render behind northern ones.
    let depth_bias  = clamp(inst.world_pos.y / 512.0, 0.0, 1.0) * 0.9;
    clip.z          = depth_bias * clip.w;
    out.clip_position = clip;
    // V57: Half-pixel inset to prevent magenta bleed from atlas cell borders
    let half_px = vec2<f32>(0.5 / 1024.0, 0.5 / 1024.0);
    let cell_min = inst.atlas_uv + half_px;
    let cell_max = inst.atlas_uv + inst.atlas_size - half_px;
    let raw_uv = inst.atlas_uv + (vertex.vertex_pos + 0.5) * inst.atlas_size;
    out.uv        = clamp(raw_uv, cell_min, cell_max);
    out.tint      = inst.tint;
    out.alpha     = inst.alpha;
    out.atlas_uv   = inst.atlas_uv;
    out.atlas_size  = inst.atlas_size;
    out.screen_size = screen_size;
    return out;
}

fn is_magenta(c: vec3<f32>) -> bool {
    // V57: Extremely aggressive magenta discard for JPEG-compressed sprites.
    // JPEG creates anti-aliased pink/rose fringes around magenta backgrounds.
    // Discard anything where red+blue dominate green significantly.
    let rb_avg = (c.r + c.b) * 0.5;
    if (rb_avg > 0.40 && c.g < rb_avg * 0.65) { return true; }
    // Also catch near-magenta with distance check
    if (distance(c, vec3<f32>(1.0, 0.0, 1.0)) < 0.5) { return true; }
    return false;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.screen_size < 2.0) { discard; }

    let c = textureSampleLevel(sprite_atlas, atlas_sampler, in.uv, 0.0);

    // Magenta chroma-key discard (#FF00FF) — algorithm based
    if (is_magenta(c.rgb)) { discard; }
    // White background discard for generated assets (AI makes off-white).
    if (c.r > 0.90 && c.g > 0.90 && c.b > 0.90) { discard; }
    // Pixel size in atlas UV space (atlas is 1024x1024)
    let px = 1.0 / 1024.0;

    if c.a < 0.1 {
        // 1px black outline: sample 4 adjacent texels, clamped to this atlas cell.
        let cell_min = in.atlas_uv;
        let cell_max = in.atlas_uv + in.atlas_size;
        let uv_n = clamp(in.uv + vec2<f32>( 0.0, -px), cell_min, cell_max);
        let uv_s = clamp(in.uv + vec2<f32>( 0.0,  px), cell_min, cell_max);
        let uv_e = clamp(in.uv + vec2<f32>( px,  0.0), cell_min, cell_max);
        let uv_w = clamp(in.uv + vec2<f32>(-px,  0.0), cell_min, cell_max);

        let sn = textureSampleLevel(sprite_atlas, atlas_sampler, uv_n, 0.0);
        let ss = textureSampleLevel(sprite_atlas, atlas_sampler, uv_s, 0.0);
        let se = textureSampleLevel(sprite_atlas, atlas_sampler, uv_e, 0.0);
        let sw = textureSampleLevel(sprite_atlas, atlas_sampler, uv_w, 0.0);

        let n = select(0.0, 1.0, sn.a > 0.5 && !is_magenta(sn.rgb));
        let s = select(0.0, 1.0, ss.a > 0.5 && !is_magenta(ss.rgb));
        let e = select(0.0, 1.0, se.a > 0.5 && !is_magenta(se.rgb));
        let w = select(0.0, 1.0, sw.a > 0.5 && !is_magenta(sw.rgb));

        if (n > 0.5 || s > 0.5 || e > 0.5 || w > 0.5) {
            return vec4<f32>(0.05, 0.03, 0.02, 0.9);
        }
        discard;
    }

    return vec4<f32>(c.rgb * in.tint, c.a * in.alpha);
}
