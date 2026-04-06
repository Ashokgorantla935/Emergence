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
    time: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};
@group(2) @binding(0) var<uniform> obj_time: ObjectTimeUniform;

// Atlas UV row for decorative objects (row 21, tree/bush sprites).
// Trees occupy UV y in [21/32, 22/32). We treat anything in that row as swayable.
const TREE_ROW_MIN: f32 = 21.0 / 32.0;
const TREE_ROW_MAX: f32 = 22.0 / 32.0;

struct VertexInput {
    @location(0) vertex_pos: vec2<f32>,
};

struct InstanceInput {
    @location(1) world_pos:   vec2<f32>,
    @location(2) atlas_uv:    vec2<f32>,
    @location(3) atlas_size:  vec2<f32>,
    @location(4) tint:        vec3<f32>,
    @location(5) size:        f32,
    @location(6) alpha:       f32,
    @location(7) _pad:        f32,
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
    let screen_size = inst.size * camera.pixels_per_unit;
    let final_size  = inst.size;  // native world-space size, no inflation
    var world_pos   = inst.world_pos;

    // Tree wind sway: only for sprites in atlas row 21 (decor tree/bush).
    // Offset only the top half of the quad (vertex_pos.y > 0) so roots stay planted.
    let in_tree_row = inst.atlas_uv.y >= TREE_ROW_MIN && inst.atlas_uv.y < TREE_ROW_MAX;
    if in_tree_row && vertex.vertex_pos.y > 0.0 {
        let sway = sin(obj_time.time * 1.5 + inst.world_pos.x * 3.0) * 0.03 * inst.size;
        world_pos.x += sway;
    }

    let world       = world_pos + vertex.vertex_pos * final_size;
    var clip        = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    // Y-sort depth bias: objects further south (higher world Y) render behind northern ones.
    let depth_bias  = clamp(inst.world_pos.y / 512.0, 0.0, 1.0) * 0.9;
    clip.z          = depth_bias * clip.w;
    out.clip_position = clip;
    out.uv        = inst.atlas_uv + (vertex.vertex_pos + 0.5) * inst.atlas_size;
    out.tint      = inst.tint;
    out.alpha     = inst.alpha;
    out.atlas_uv   = inst.atlas_uv;
    out.atlas_size  = inst.atlas_size;
    out.screen_size = screen_size;
    return out;
}

fn is_magenta(c: vec3<f32>) -> bool {
    return (c.r > 0.5 && c.b > 0.5 && c.g < 0.45) || distance(c, vec3<f32>(1.0, 0.0, 1.0)) < 0.8;
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
