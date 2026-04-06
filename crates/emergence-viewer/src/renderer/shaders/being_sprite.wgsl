// Sprite shader for being instances.
// Renders instanced billboard quads sampling the 1024x1024 pixel-art atlas.
// Enforces 8px minimum screen size so beings are never invisible dots.
// Walking bob: sine-wave Y offset only when moving (bob_flip != 0).
// Outline: 1px black border sampled from neighboring texels.
// Shadow: elliptical oval beneath each sprite.

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

struct TimeUniform {
    time: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};
@group(2) @binding(0) var<uniform> time_u: TimeUniform;

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
    /// Encoded: sign = facing dir (+1 right, -1 left). Magnitude = bob phase.
    /// Exactly 0.0 means idle (no bob, no flip needed — default face right).
    @location(9) bob_flip:     f32,
};

// ── Vertex output ────────────────────────────────────────────────────────────

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv:           vec2<f32>,
    @location(1) emotion_tint: vec3<f32>,
    @location(2) skin_tone:    vec3<f32>,
    @location(3) brightness:   f32,
    @location(4) alpha:        f32,
    @location(5) local_v:      f32,   // vertex V in [0,1]: 0=top, 1=bottom
    @location(6) local_u:      f32,   // vertex U in [0,1]: 0=left, 1=right
    @location(7) atlas_uv:     vec2<f32>,  // passed through for outline sampling
    @location(8) atlas_size:   vec2<f32>,  // passed through for outline sampling
    @location(9) screen_size:  f32,
};

// ── Vertex shader ────────────────────────────────────────────────────────────

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    let screen_size = instance.size * camera.pixels_per_unit;
    let final_size  = instance.size;  // native world-space size, no inflation

    // Walking bob: only when bob_flip != 0 (moving).
    // bob_flip encodes: sign = facing direction, magnitude = phase (game_tick * 0.18 + id * 0.72).
    // Amplitude 1.25 world pixels = 1.25 / pixels_per_unit world units.
    let bob_amplitude = 1.25 / max(camera.pixels_per_unit, 1.0);
    var bob_offset = 0.0;
    if (abs(instance.bob_flip) > 0.001) {
        let phase = abs(instance.bob_flip);
        bob_offset = sin(phase) * bob_amplitude;
    }

    // Horizontal flip: if bob_flip < 0, facing left — mirror UV horizontally.
    let flip_sign = select(1.0, -1.0, instance.bob_flip < -0.001);

    // Billboard quad centred on world_pos, shifted up by bob.
    var pos = instance.world_pos;
    pos.y += bob_offset;
    let world_xy = pos + vertex.vertex_pos * final_size;
    var clip_pos = camera.view_proj * vec4<f32>(world_xy, 0.0, 1.0);
    // Y-sort depth bias: beings further south (higher world Y) render behind northern ones.
    let depth_bias = clamp(instance.world_pos.y / 512.0, 0.0, 1.0) * 0.9;
    clip_pos.z = depth_bias * clip_pos.w;
    out.clip_position = clip_pos;

    // Map vertex [-0.5,0.5] to atlas UV within the cell.
    // Apply horizontal flip by mirroring the U component around cell center.
    let local_uv = vertex.vertex_pos + vec2<f32>(0.5, 0.5); // [0,1]
    // Flip U: if flip_sign = -1, u becomes 1 - u within cell
    let flipped_u = select(local_uv.x, 1.0 - local_uv.x, flip_sign < 0.0);
    let cell_uv   = vec2<f32>(flipped_u, local_uv.y);
    out.uv           = instance.atlas_uv + cell_uv * instance.atlas_size;
    out.atlas_uv     = instance.atlas_uv;
    out.atlas_size   = instance.atlas_size;
    out.emotion_tint = instance.emotion_tint;
    out.skin_tone    = instance.skin_tone;
    out.brightness   = instance.brightness;
    out.alpha        = instance.alpha;
    // vertex_pos.y = -0.5 at bottom, +0.5 at top → local_v 1.0 at bottom, 0.0 at top
    out.local_v      = 0.5 - vertex.vertex_pos.y;
    out.local_u      = local_uv.x;
    out.screen_size  = screen_size;
    return out;
}

// ── Fragment shader ──────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // LOD 2 sentinel: atlas_size.x < 0.001 means macro-zoom solid-color dot.
    // Skip atlas sampling entirely — render emotion_tint as a filled circle.
    if (in.atlas_size.x < 0.001) {
        // Circular dot shape: distance from centre of quad.
        let cx = (in.local_u - 0.5) * 2.0; // -1..1
        let cy = (in.local_v - 0.5) * 2.0; // -1..1
        let dist_sq = cx * cx + cy * cy;
        if (dist_sq > 1.0) { discard; }
        let final_rgb = in.emotion_tint * in.brightness;
        return vec4<f32>(final_rgb, in.alpha);
    }
    if (in.screen_size < 2.0) { discard; }
    var texel = textureSampleLevel(sprite_atlas, atlas_sampler, in.uv, 0.0);
    
    // Magenta chroma-key discard (#FF00FF) — relative checking handles anti-aliased AI fringes
    if (texel.r > texel.g + 0.15 && texel.b > texel.g + 0.15) { discard; }
    
    // White background discard for generated fauna assets
    if (texel.r > 0.95 && texel.g > 0.95 && texel.b > 0.95) { discard; }

    let alpha = texel.a;

    // Pixel size in atlas UV space
    let atlas_dim = vec2<f32>(textureDimensions(sprite_atlas));
    let px = 1.0 / atlas_dim;

    // Transparent pixel: check for outline or shadow before discarding.
    if (alpha < 0.1) {
        // 1px black outline: sample 4 adjacent texels in atlas space.
        let sn = textureSampleLevel(sprite_atlas, atlas_sampler, in.uv + vec2<f32>(0.0,  -px.y), 0.0);
        let ss = textureSampleLevel(sprite_atlas, atlas_sampler, in.uv + vec2<f32>(0.0,   px.y), 0.0);
        let se = textureSampleLevel(sprite_atlas, atlas_sampler, in.uv + vec2<f32>( px.x,  0.0), 0.0);
        let sw = textureSampleLevel(sprite_atlas, atlas_sampler, in.uv + vec2<f32>(-px.x,  0.0), 0.0);

        let nm = sn.r > sn.g + 0.15 && sn.b > sn.g + 0.15;
        let sm = ss.r > ss.g + 0.15 && ss.b > ss.g + 0.15;
        let em = se.r > se.g + 0.15 && se.b > se.g + 0.15;
        let wm = sw.r > sw.g + 0.15 && sw.b > sw.g + 0.15;

        let nw = sn.r > 0.95 && sn.g > 0.95 && sn.b > 0.95;
        let sw_w = ss.r > 0.95 && ss.g > 0.95 && ss.b > 0.95;
        let ew = se.r > 0.95 && se.g > 0.95 && se.b > 0.95;
        let ww = sw.r > 0.95 && sw.g > 0.95 && sw.b > 0.95;

        let n = select(0.0, 1.0, sn.a > 0.5 && !nm && !nw);
        let s = select(0.0, 1.0, ss.a > 0.5 && !sm && !sw_w);
        let e = select(0.0, 1.0, se.a > 0.5 && !em && !ew);
        let w = select(0.0, 1.0, sw.a > 0.5 && !wm && !ww);

        if (n > 0.5 || s > 0.5 || e > 0.5 || w > 0.5) {
            return vec4<f32>(0.05, 0.03, 0.02, 0.9);
        }

        // Elliptical ground shadow: bottom 15% of quad, oval shape.
        // local_v: 0=top, 1=bottom. local_u: 0=left, 1=right.
        let shadow_y = (in.local_v - 0.85) / 0.15; // ramps 0→1 in bottom 15%
        if (shadow_y > 0.0 && shadow_y < 1.0) {
            let shadow_x = (in.local_u - 0.5) * 2.0; // -1 to 1
            let dist = shadow_x * shadow_x + shadow_y * shadow_y;
            if (dist < 1.0) {
                return vec4<f32>(0.0, 0.0, 0.0, 0.4 * (1.0 - dist));
            }
        }

        discard;
    }

    // Skin and clothing tinting (opaque pixels only).
    if (texel.a > 0.1) {
        // Skin detection: pixels close to default skin tone get tinted toward being's skin_tone.
        let default_skin = vec3<f32>(1.0, 0.835, 0.639);
        let skin_dist = length(texel.rgb - default_skin);
        if (skin_dist < 0.25) {
            let blend = 1.0 - (skin_dist / 0.25);
            texel = vec4<f32>(mix(texel.rgb, in.skin_tone, blend * 0.6), texel.a);
        }

        // Clothing detection: pixels close to default outfit color (purple/blue) tint to emotion.
        let default_outfit = vec3<f32>(0.545, 0.388, 0.757);
        let outfit_dist = length(texel.rgb - default_outfit);
        if (outfit_dist < 0.30) {
            let blend = 1.0 - (outfit_dist / 0.30);
            texel = vec4<f32>(mix(texel.rgb, in.emotion_tint, blend * 0.5), texel.a);
        }
    }

    // Opaque/semi-opaque pixel: apply brightness and per-instance alpha.
    let final_rgb = texel.rgb * in.brightness;

    return vec4<f32>(final_rgb, alpha * in.alpha);
}
