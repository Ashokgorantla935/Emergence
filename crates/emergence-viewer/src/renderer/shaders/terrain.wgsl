// Instanced quad terrain shader — each terrain cell is a textured quad
// sampling a real 16x16 tile from the sprite atlas.

struct CameraUniform {
    view_proj:       mat4x4<f32>,
    pixels_per_unit: f32,
    _pad0:           f32,
    _pad1:           f32,
    zoom_level:      u32,  // 0=macro(>150 cells), 1=medium(50-150), 2=close(<50)
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
    @location(4) flags:     f32,       // biome id: 0=grass, 1=water, 2=forest, 3=desert, 4=mountain, 5=wetland
    @location(5) _pad:      f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) atlas_uv:  vec2<f32>,
    @location(1) flags:     f32,
    @location(2) world_pos: vec2<f32>, // integer cell coords
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
    // Apply 0.5px inset (0.001 in UV space for 16px tile in 512px atlas) to prevent bleeding
    let tile_size = 1.0 / 32.0; // 16px / 512px atlas
    let inset = 0.001; // ~0.5px inset at 512px atlas
    let uv_min = inst.tile_uv + vec2<f32>(inset, inset);
    let uv_range = tile_size - 2.0 * inset;
    out.atlas_uv = uv_min + vertex.uv * uv_range;
    out.flags = inst.flags;
    out.world_pos = inst.world_pos; // pass cell origin (integer coords)

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

// Per-cell brightness variation using cell coords (stable, not per-pixel).
fn cell_brightness(world_pos: vec2<f32>) -> f32 {
    let h = fract(sin(dot(world_pos, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    return 0.97 + h * 0.06; // [0.97, 1.03] — +/- 3% per cell
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = water_time.time;
    let biome_id = u32(in.flags + 0.5);
    let is_water = (biome_id == 1u);

    let base_color = biome_base_color(biome_id);

    // ── LOD 0: Macro zoom — flat solid biome color only ───────────────────
    if (camera.zoom_level == 0u) {
        return base_color;
    }

    // ── LOD 1: Medium zoom — base + subtle per-cell variation ─────────────
    if (camera.zoom_level == 1u) {
        let variation = cell_brightness(in.world_pos);
        var color = vec4<f32>(base_color.rgb * variation, 1.0);

        if (is_water) {
            // Animated brightness pulse only — no atlas sampling
            let pulse = sin(t * 2.0 + in.world_pos.x * 0.5) * 0.03;
            color = vec4<f32>(
                clamp(color.r + pulse,        0.0, 1.0),
                clamp(color.g + pulse,        0.0, 1.0),
                clamp(color.b + pulse * 1.5,  0.0, 1.0),
                1.0,
            );
        } else {
            // Signal tinting for land
            let d = clamp(water_time.signal_danger,  0.0, 1.0);
            let c = clamp(water_time.signal_comfort, 0.0, 1.0);
            let g = clamp(water_time.signal_grief,   0.0, 1.0);
            color = vec4<f32>(
                clamp(color.r * (1.0 + d * 0.10 - g * 0.10), 0.0, 1.0),
                clamp(color.g * (1.0 - d * 0.10 - g * 0.10), 0.0, 1.0),
                clamp(color.b * (1.0 - d * 0.10 + c * (-0.05) + g * 0.10), 0.0, 1.0),
                1.0,
            );

            // --- Decoration overlay at LOD 1 (medium zoom, sparse dots) ---
            let cell = floor(in.world_pos);
            let h  = fract(sin(dot(cell, vec2<f32>(12.9898, 78.233))) * 43758.5453);
            let h2 = fract(sin(dot(cell, vec2<f32>(63.7264, 10.873))) * 43758.5453);
            let cell_frac = fract(in.world_pos) - vec2<f32>(0.5, 0.5);
            let dist = length(cell_frac);

            // Forest: tree dot (dark green)
            if (biome_id == 2u && h < 0.33 && dist < 0.18) {
                let green = 0.22 + h2 * 0.12;
                color = mix(color, vec4<f32>(0.08, green, 0.04, 1.0), 0.75);
            }
            // Mountain: rock dot (grey)
            if (biome_id == 4u && h < 0.18 && dist < 0.16) {
                let grey = 0.38 + h2 * 0.18;
                color = mix(color, vec4<f32>(grey, grey, grey * 0.94, 1.0), 0.55);
            }
            // Desert: cactus dot (muted green)
            if (biome_id == 3u && h < 0.10 && dist < 0.10) {
                color = mix(color, vec4<f32>(0.22, 0.46, 0.14, 1.0), 0.65);
            }
        }
        return color;
    }

    // ── LOD 2: Close zoom — full detail with atlas overlay ────────────────
    let variation = cell_brightness(in.world_pos);
    var color = vec4<f32>(base_color.rgb * variation, 1.0);

    if (is_water) {
        // Water: solid blue base + wave brightness animation — NO atlas sampling
        let pulse = sin(t * 2.0) * 0.05;
        color = vec4<f32>(
            clamp(color.r + pulse,       0.0, 1.0),
            clamp(color.g + pulse,       0.0, 1.0),
            clamp(color.b + pulse * 1.5, 0.0, 1.0),
            1.0,
        );
    } else {
        // Land: solid biome color + signal tinting + decorations — NO atlas sampling

        // Signal tinting
        let d = clamp(water_time.signal_danger,  0.0, 1.0);
        let c = clamp(water_time.signal_comfort, 0.0, 1.0);
        let g = clamp(water_time.signal_grief,   0.0, 1.0);
        color = vec4<f32>(
            clamp(color.r * (1.0 + d * 0.10 - g * 0.10), 0.0, 1.0),
            clamp(color.g * (1.0 - d * 0.10 - g * 0.10), 0.0, 1.0),
            clamp(color.b * (1.0 - d * 0.10 + c * (-0.05) + g * 0.10), 0.0, 1.0),
            1.0,
        );

        // --- Decoration overlay at LOD 2 (close zoom, full detail) ---
        // Deterministic hash per cell — no flicker
        let cell = floor(in.world_pos);
        let h  = fract(sin(dot(cell, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        let h2 = fract(sin(dot(cell, vec2<f32>(63.7264, 10.873))) * 43758.5453);

        // Forest: tree canopy blob with trunk center
        if (biome_id == 2u && h < 0.33) {
            let offset = vec2<f32>(0.3 + h2 * 0.4, 0.3 + h * 0.4);
            let cell_frac = fract(in.world_pos) - offset;
            let dist = length(cell_frac);
            if (dist < 0.35) {
                let green = 0.25 + h2 * 0.15;
                var decor = vec4<f32>(0.1, green, 0.05, 1.0);
                if (dist < 0.08) {
                    // Trunk: brown center
                    decor = vec4<f32>(0.35, 0.22, 0.1, 1.0);
                }
                color = mix(color, decor, 0.85);
            }
        }

        // Grassland: small flower or grass tuft
        if (biome_id == 0u && h < 0.12) {
            let cell_frac = fract(in.world_pos) - vec2<f32>(0.5, 0.5);
            let dist = length(cell_frac);
            if (dist < 0.15) {
                let flower_r = 0.4 + h2 * 0.3;
                let flower_g = 0.6 + h2 * 0.3;
                color = mix(color, vec4<f32>(flower_r, flower_g, 0.2, 1.0), 0.7);
            }
        }

        // Wetland: reed tuft (dark yellow-green vertical smear)
        if (biome_id == 5u && h < 0.20) {
            let cell_frac = fract(in.world_pos) - vec2<f32>(0.5 + h2 * 0.2 - 0.1, 0.5);
            if (abs(cell_frac.x) < 0.05 && cell_frac.y > -0.22 && cell_frac.y < 0.22) {
                color = mix(color, vec4<f32>(0.30, 0.42, 0.08, 1.0), 0.75);
            }
        }

        // Desert: cactus — vertical green bar with arms
        if (biome_id == 3u && h < 0.08) {
            let cx = 0.5 + h2 * 0.2 - 0.1;
            let cell_frac = fract(in.world_pos) - vec2<f32>(cx, 0.4 + h2 * 0.2);
            // Trunk
            if (abs(cell_frac.x) < 0.06 && cell_frac.y > -0.20 && cell_frac.y < 0.20) {
                color = mix(color, vec4<f32>(0.2, 0.5, 0.15, 1.0), 0.80);
            }
            // Left arm
            if (cell_frac.x > -0.18 && cell_frac.x < -0.06 && abs(cell_frac.y + 0.05) < 0.05) {
                color = mix(color, vec4<f32>(0.2, 0.5, 0.15, 1.0), 0.80);
            }
            // Right arm
            if (cell_frac.x > 0.06 && cell_frac.x < 0.18 && abs(cell_frac.y - 0.05) < 0.05) {
                color = mix(color, vec4<f32>(0.2, 0.5, 0.15, 1.0), 0.80);
            }
        }

        // Mountain: rock blob (grey, irregular)
        if (biome_id == 4u && h < 0.15) {
            let cell_frac = fract(in.world_pos) - vec2<f32>(0.5, 0.5);
            let dist = length(cell_frac);
            if (dist < 0.20) {
                let grey = 0.40 + h2 * 0.20;
                color = mix(color, vec4<f32>(grey, grey, grey * 0.95, 1.0), 0.60);
            }
        }
    }

    return color;
}
