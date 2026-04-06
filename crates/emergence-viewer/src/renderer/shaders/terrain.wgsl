// Instanced quad terrain shader — each terrain cell is a textured quad
// sampling a real 16x16 tile from the sprite atlas.

struct CameraUniform {
    view_proj:       mat4x4<f32>,
    pixels_per_unit: f32,
    _pad0:           f32,
    _pad1:           f32,
    zoom_blend:      f32,  // 0.0=LOD0(macro), 1.0=LOD1(medium), 2.0=LOD2(close); fractional=blend
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var t_atlas: texture_2d<f32>;
@group(1) @binding(1) var s_atlas: sampler;

// group 2: water time + signal tint + day/night illumination uniform
// Layout: vec4 [time, signal_danger, signal_comfort, signal_grief]
//       + vec4 [illumination, water_level, _pad2, _pad3]
struct WaterTime {
    time:           f32,
    signal_danger:  f32,
    signal_comfort: f32,
    signal_grief:   f32,
    illumination:   f32, // day/night: 0.0 = full night, 1.0 = full day
    water_level:    f32, // dynamic sea level: base 0.28 + water_level_offset
    _wt_pad2:       f32,
    _wt_pad3:       f32,
};
@group(2) @binding(0) var<uniform> water_time: WaterTime;

struct VertexInput {
    @location(0) position: vec2<f32>,  // unit quad corner (0-1)
    @location(1) uv:       vec2<f32>,  // 0-1 within quad
};

struct InstanceInput {
    @location(2) world_pos:      vec2<f32>, // world x, y of this cell
    @location(3) tile_uv:        vec2<f32>, // UV origin in the atlas for this tile
    @location(4) flags:          f32,       // biome id: 0=grass, 1=water, 2=forest, 3=desert, 4=mountain, 5=wetland
    @location(5) elevation:      f32,       // terrain elevation [0.0, 1.0]
    @location(6) structure_type: f32,       // 0=None, 1=Campfire, 2=LeanTo, 3=Hut, 4=Wall, 5=ResourceCache
    @location(7) build_progress: f32,       // Construction ticks
    @location(8) density:        f32,       // V54 §4.1: flora/entity density [0.0, 1.0] for canopy shadow
    @location(9) _pad_density:   f32,       // padding
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) atlas_uv:       vec2<f32>,
    @location(1) @interpolate(flat) flags:          f32,
    @location(2) world_pos:      vec2<f32>, // integer cell coords
    @location(3) @interpolate(flat) structure_type: f32,
    @location(4) @interpolate(flat) elevation:      f32,       // terrain elevation [0.0, 1.0]
    @location(5) @interpolate(flat) build_progress: f32,
    @location(6) tile_uv:        vec2<f32>,
    @location(7) uv:             vec2<f32>, // tile-local [0,1] UV — interpolates cleanly inside quad
    @location(8) @interpolate(flat) density: f32, // V54 §4.1: flora density for canopy shadow
};

@vertex
fn vs_main(vertex: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // Direct 1-to-1 cell mapping
    let world = vec2<f32>(
        inst.world_pos.x + vertex.position.x,
        inst.world_pos.y + vertex.position.y,
    );

    out.clip_position = camera.view_proj * vec4<f32>(world, 0.0, 1.0);

    // Tile-local UV interpolates perfectly [0,1] across the quad — no seam artifacts
    out.uv = vertex.uv;

    // Provide base tile UV to bound the fragment samples when generating the canopy layers
    out.tile_uv = inst.tile_uv;

    // Disable atlas binding for base terrain to emulate WorldBox solid colors.
    // Trees, mountains and rocks are rendered via ObjectRenderer.
    out.atlas_uv = vec2<f32>(0.0, 0.0);

    out.flags = inst.flags;
    out.world_pos = world;
    out.structure_type = inst.structure_type;
    out.elevation = inst.elevation;
    out.build_progress = inst.build_progress;
    out.density = inst.density;
    return out;
}

// Returns the WorldBox-palette solid base color for a biome id.
fn biome_base_color(biome_id: u32) -> vec4<f32> {
    switch biome_id {
        case 0u: { return vec4<f32>(0.667, 0.741, 0.239, 1.0); } // #aabd3d grassland
        case 1u: { return vec4<f32>(0.0,   0.471, 0.945, 1.0); } // #0078f1 water
        case 2u: { return vec4<f32>(0.133, 0.773, 0.365, 1.0); } // #22C55E forest
        case 3u: { return vec4<f32>(0.988, 0.827, 0.471, 1.0); } // #FCD34D desert
        case 4u: { return vec4<f32>(0.612, 0.639, 0.686, 1.0); } // #9CA3AF mountain
        case 5u: { return vec4<f32>(0.404, 0.545, 0.0,   1.0); } // #678b00 wetland/swamp
        case 6u: { return vec4<f32>(0.95,  0.95,  0.98,  1.0); } // snow
        case 7u: { return vec4<f32>(0.6,   0.65,  0.55,  1.0); } // tundra
        default: { return vec4<f32>(0.667, 0.741, 0.239, 1.0); } // fallback grassland
    }
}

// Smooth organic pixel-fractal noise without f32 sin() precision loss at large coords.
// Uses Wang hashing at integer grid vertices perfectly smoothly interpolating.
fn hash(p: vec2<u32>) -> f32 {
    var h: u32 = p.x * 2654435761u ^ p.y * 2246822519u;
    h = (h >> 16u) ^ h;
    h = h * 2654435761u;
    h = (h >> 16u) ^ h;
    return f32(h & 0xFFFFu) / 65535.0;
}

fn organic_noise(p: vec2<f32>) -> f32 {
    let pi = vec2<u32>(u32(i32(floor(p.x))), u32(i32(floor(p.y))));
    let pf = fract(p);
    
    // Smoothstep interpolation
    let u = pf * pf * (3.0 - 2.0 * pf);
    
    let a = hash(pi + vec2<u32>(0u, 0u));
    let b = hash(pi + vec2<u32>(1u, 0u));
    let c = hash(pi + vec2<u32>(0u, 1u));
    let d = hash(pi + vec2<u32>(1u, 1u));
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn cell_brightness(world_pos: vec2<f32>) -> f32 {
    // Large organic swoops mixed with micro-organic details
    let n1 = organic_noise(world_pos * 0.4);
    let n2 = organic_noise(world_pos * 1.5);
    let combined = n1 * 0.7 + n2 * 0.3;
    
    // Instead of jumping by 6% abruptly per square grid block,
    // this flows organically across pixels: [-0.03, +0.03].
    return 0.94 + combined * 0.12; 
}

// Water depth color based on elevation (water threshold ~0.25-0.30 in terrain gen).
// Maps elevation to a depth gradient: deep navy → medium blue → cyan → shore foam.
fn water_depth_color(elevation: f32) -> vec4<f32> {
    if (elevation < 0.08) {
        // Deep water — dark navy #0a1628
        return vec4<f32>(0.039, 0.086, 0.157, 1.0);
    } else if (elevation < 0.18) {
        // Medium depth — navy to medium blue #1a3a6a
        let t = (elevation - 0.08) / 0.10;
        return mix(
            vec4<f32>(0.039, 0.086, 0.157, 1.0),
            vec4<f32>(0.102, 0.227, 0.416, 1.0),
            t,
        );
    } else if (elevation < 0.25) {
        // Shallow — medium blue to bright cyan #00bcd4
        let t = (elevation - 0.18) / 0.07;
        return mix(
            vec4<f32>(0.102, 0.227, 0.416, 1.0),
            vec4<f32>(0.0,   0.737, 0.831, 1.0),
            t,
        );
    } else {
        // Shore fringe — cyan to white foam
        let t = clamp((elevation - 0.25) / 0.05, 0.0, 1.0);
        return mix(
            vec4<f32>(0.0,   0.737, 0.831, 1.0),
            vec4<f32>(0.90,  0.95,  1.0,   1.0),
            t,
        );
    }
}

// Apply day/night illumination. Night dims to 20% brightness.
// Settlement glow: high comfort signal adds local light at night (proxy for inhabited areas).
fn apply_illumination(color: vec4<f32>, illumination: f32, comfort: f32) -> vec4<f32> {
    let settle_light = clamp(comfort * 0.4, 0.0, 0.4) * (1.0 - illumination);
    let effective_light = clamp(illumination + settle_light, 0.0, 1.0);
    let dim = mix(0.2, 1.0, effective_light);
    return vec4<f32>(color.rgb * dim, color.a);
}

// Structure overlays. Mixes building color onto the base biome color based on cell distance and LOD
fn apply_structure(base: vec4<f32>, structure_type: u32, build_progress: f32, world_pos: vec2<f32>, time: f32, lod: u32, uv: vec2<f32>) -> vec4<f32> {
    // Use tile-local uv [0,1] mapped to [-0.5, 0.5] — avoids fract(world_pos) triangle-border tearing
    let cell_frac = uv - vec2<f32>(0.5, 0.5);

    // Create a smooth squircle mask to confine the road to the cell geometry without hitting triangle edges
    let rect_dist = max(abs(cell_frac.x), abs(cell_frac.y));
    let mask = smoothstep(0.50, 0.40, rect_dist);

    // 1-5, 8-20 are handled by 3D object sprites (objects.rs)

    // DirtPath (6) — worn earth trail
    if (structure_type == 6u) {
        let dirt = vec4<f32>(0.62, 0.49, 0.32, 1.0);
        return mix(base, dirt, mask * 0.70);
    }

    // StoneRoad (7) — cobblestone road
    if (structure_type == 7u) {
        let stone_road = vec4<f32>(0.55, 0.55, 0.53, 1.0);
        // Use smooth sine waves to create grid pattern instead of fract, which avoids 
        // bounding box precision tearing across the diagonal vertices in the quad.
        let grid_x = smoothstep(-0.2, 0.2, sin(uv.x * 3.14159 * 8.0));
        let grid_y = smoothstep(-0.2, 0.2, sin(uv.y * 3.14159 * 8.0));
        let cobble = mix(vec4<f32>(0.45, 0.45, 0.43, 1.0), stone_road, grid_x * grid_y);
        return mix(base, cobble, mask * 0.90);
    }

    // FarmField (20) — handled by 3D object sprites

    return base;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = water_time.time;
    let biome_id = u32(in.flags + 0.5);
    var is_water = (biome_id == 1u);
    // Dynamic sea level: cells whose elevation is below the current water level are flooded.
    let dynamic_water_level = water_time.water_level;
    if (!is_water && dynamic_water_level > 0.0 && in.elevation < dynamic_water_level) {
        is_water = true;
    }
    let illumination = clamp(water_time.illumination, 0.0, 1.0);
    let comfort = clamp(water_time.signal_comfort, 0.0, 1.0);

    // Sample the 16x16 seamless terrain spritesheet
    let ATLAS_CELL = 1.0 / 16.0;
    // Map the local quad UV to the spritesheet cell bounding box
    let sample_uv = in.tile_uv + in.uv * ATLAS_CELL;
    let base_color = textureSample(t_atlas, s_atlas, sample_uv);
    let structure_id = u32(in.structure_type + 0.5);

    // zoom_blend: 0.0=LOD0, 1.0=LOD1, 2.0=LOD2; fractional = smooth blend between adjacent LODs
    let zoom = clamp(camera.zoom_blend, 0.0, 2.0);

    // ── LOD 0: Macro zoom — flat solid biome color ────────────────────────
    var color_lod0 = base_color;
    if (is_water) {
        color_lod0 = water_depth_color(in.elevation);
    } else {
        // V54 §4.1: Macro-density canopy shadow for LOD-culled flora.
        // At macro zoom (LOD0), individual tree sprites are sub-pixel and culled.
        // Darken terrain proportional to flora/entity density to simulate canopy cover.
        let canopy_shadow = in.density * smoothstep(1.0, 0.0, zoom) * 0.40;
        color_lod0 = mix(color_lod0, vec4<f32>(0.05, 0.12, 0.03, 1.0), canopy_shadow);
    }

    // ── LOD 1: Medium zoom — base + per-cell brightness + signal tinting ──
    let variation = cell_brightness(in.world_pos);
    let d = clamp(water_time.signal_danger,  0.0, 1.0);
    let c = clamp(water_time.signal_comfort, 0.0, 1.0);
    let g = clamp(water_time.signal_grief,   0.0, 1.0);
    var color_lod1: vec4<f32>;
    if (is_water) {
        let depth_color = water_depth_color(in.elevation);
        let pulse = sin(t * 2.0 + fract(in.world_pos.x * 0.5) * 6.283185) * 0.03;
        color_lod1 = vec4<f32>(
            clamp(depth_color.r + pulse,        0.0, 1.0),
            clamp(depth_color.g + pulse,        0.0, 1.0),
            clamp(depth_color.b + pulse * 1.5,  0.0, 1.0),
            1.0,
        );
    } else {
        let lod1_base = vec4<f32>(base_color.rgb * variation, 1.0);
        color_lod1 = vec4<f32>(
            clamp(lod1_base.r * (1.0 + d * 0.10 - g * 0.10), 0.0, 1.0),
            clamp(lod1_base.g * (1.0 - d * 0.10 - g * 0.10), 0.0, 1.0),
            clamp(lod1_base.b * (1.0 - d * 0.10 + c * (-0.05) + g * 0.10), 0.0, 1.0),
            1.0,
        );
    }

    // Blend LOD 0 → LOD 1 when zoom in [0, 1)
    if (zoom < 1.0) {
        let blended = mix(color_lod0, color_lod1, zoom);
        let structured = apply_structure(blended, structure_id, in.build_progress, in.world_pos, t, 0u, in.uv);
        return apply_illumination(structured, illumination, comfort);
    }

    // ── LOD 2: Close zoom — LOD 1 + shore foam + forest canopy ───────────
    var color_lod2: vec4<f32>;
    if (is_water) {
        let depth_color = water_depth_color(in.elevation);
        let pulse = sin(t * 2.0) * 0.05;
        color_lod2 = vec4<f32>(
            clamp(depth_color.r + pulse,       0.0, 1.0),
            clamp(depth_color.g + pulse,       0.0, 1.0),
            clamp(depth_color.b + pulse * 1.5, 0.0, 1.0),
            1.0,
        );
        // Shore foam: layered crashing waves
        if (in.elevation > 0.20 && in.elevation < 0.28) {
            let shore_dist = clamp((0.28 - in.elevation) / 0.08, 0.0, 1.0);
            let wave_time = t * 2.0 - shore_dist * 8.0;
            let wave_form = fract(wave_time);
            let foam_line = step(0.90, wave_form) * (1.0 - wave_form) * 10.0;
            let static_foam = clamp((in.elevation - 0.25) / 0.03, 0.0, 1.0) * 0.4;
            let foam_mix = clamp((foam_line * (1.0 - shore_dist)) + static_foam, 0.0, 1.0);
            color_lod2 = mix(color_lod2, vec4<f32>(0.92, 0.98, 1.0, 1.0), foam_mix * 0.9);
        }
    } else {
        let lod2_base = vec4<f32>(base_color.rgb * variation, 1.0);
        color_lod2 = vec4<f32>(
            clamp(lod2_base.r * (1.0 + d * 0.10 - g * 0.10), 0.0, 1.0),
            clamp(lod2_base.g * (1.0 - d * 0.10 - g * 0.10), 0.0, 1.0),
            clamp(lod2_base.b * (1.0 - d * 0.10 + c * (-0.05) + g * 0.10), 0.0, 1.0),
            1.0,
        );
        // Multi-layer canopy foliage for forests (procedural only)
        if (biome_id == 2u) {
            let sway = sin(t * 1.5 + fract(in.world_pos.x * 0.5) * 6.283185) * 0.05;
            let canopy_darkness = 0.15 + (sin(fract(in.world_pos.x * 3.0) * 6.283185 + t + sway) * cos(fract(in.world_pos.y * 3.0) * 6.283185)) * 0.05;
            color_lod2 = mix(color_lod2, vec4<f32>(0.1, 0.3, 0.1, 1.0), canopy_darkness);
        }
    }

    // Blend LOD 1 → LOD 2 when zoom in [1, 2]
    let blend_12 = zoom - 1.0;
    let blended = mix(color_lod1, color_lod2, blend_12);
    let structured = apply_structure(blended, structure_id, in.build_progress, in.world_pos, t, u32(zoom), in.uv);
    return apply_illumination(structured, illumination, comfort);
}
