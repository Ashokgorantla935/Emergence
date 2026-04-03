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
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) atlas_uv:       vec2<f32>,
    @location(1) flags:          f32,
    @location(2) world_pos:      vec2<f32>, // integer cell coords
    @location(3) structure_type: f32,
    @location(4) elevation:      f32,       // terrain elevation [0.0, 1.0]
    @location(5) build_progress: f32,
    @location(6) tile_uv:        vec2<f32>,
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
    let p = floor(world_pos);
    let h = fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    return 0.97 + h * 0.06; // [0.97, 1.03] — +/- 3% per cell
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
fn apply_structure(base: vec4<f32>, structure_type: u32, build_progress: f32, world_pos: vec2<f32>, time: f32, lod: u32) -> vec4<f32> {
    let cell_frac = fract(world_pos) - vec2<f32>(0.5, 0.5); // [-0.5, 0.5]
    let dist = length(cell_frac);

    // Scaffolding when under construction
    if (structure_type == 0u && build_progress > 0.0) {
        if (lod > 0u) {
            let scaffold_color = vec4<f32>(0.7, 0.6, 0.4, 1.0); // wood color
            
            // X shape 
            let d1 = abs(cell_frac.x - cell_frac.y);
            let d2 = abs(cell_frac.x + cell_frac.y);
            
            // Limit construction scaffold size based on progress
            let max_dist = 0.15 + (build_progress * 0.005); // Grows slowly
            if ((d1 < 0.05 || d2 < 0.05) && dist < max_dist) {
                return mix(base, scaffold_color, 0.85);
            }
        }
        return base;
    }

    // Campfire (1) — orange/red warm dot with animated flicker
    if (structure_type == 1u) {
        if (lod == 0u) {
            if (dist < 0.35) {
                return mix(base, vec4<f32>(1.0, 0.45, 0.05, 1.0), 0.85);
            }
        } else if (lod == 1u) {
            let flicker = 0.85 + sin(time * 8.0 + world_pos.x) * 0.15;
            if (dist < 0.22) {
                let fire = vec4<f32>(1.0 * flicker, 0.35 * flicker, 0.05, 1.0);
                return mix(base, fire, 0.90);
            }
            // Warm glow radius
            if (dist < 0.38) {
                return mix(base, vec4<f32>(1.0, 0.60, 0.10, 1.0), 0.25);
            }
        } else {
            // LOD 2: animated flame with ember core
            let flicker = 0.80 + sin(time * 12.0 + world_pos.x * 3.0) * 0.20;
            if (dist < 0.10) {
                // Bright ember core
                return mix(base, vec4<f32>(1.0, 0.9, 0.3, 1.0), 0.95);
            }
            if (dist < 0.20) {
                let fire = vec4<f32>(1.0 * flicker, 0.3 * flicker, 0.02, 1.0);
                return mix(base, fire, 0.90);
            }
            if (dist < 0.38) {
                return mix(base, vec4<f32>(1.0, 0.55, 0.08, 1.0), 0.30);
            }
        }
    }

    // LeanTo (2) — brown triangle shelter
    if (structure_type == 2u) {
        let brown = vec4<f32>(0.545, 0.271, 0.075, 1.0); // #8B4513
        if (lod == 0u) {
            if (dist < 0.35) {
                return mix(base, brown, 0.85);
            }
        } else if (lod == 1u) {
            // Rough triangle: top half of cell, wider at bottom
            let fx = cell_frac.x;
            let fy = cell_frac.y;
            // Triangle: peak at (0, 0.3), base from (-0.3, -0.2) to (0.3, -0.2)
            let in_triangle = (fy > -0.25) && (fy < 0.30) && (abs(fx) < (0.30 - fy) * 0.90);
            if (in_triangle) {
                return mix(base, brown, 0.80);
            }
        } else {
            // LOD 2: triangle with darker edge outline
            let fx = cell_frac.x;
            let fy = cell_frac.y;
            let half_w = (0.30 - fy) * 0.90;
            let in_triangle = (fy > -0.25) && (fy < 0.30) && (abs(fx) < half_w);
            let on_edge = in_triangle && (abs(fx) > half_w - 0.05 || fy > 0.25 || fy < -0.20);
            if (on_edge) {
                return mix(base, vec4<f32>(0.25, 0.12, 0.03, 1.0), 0.90);
            }
            if (in_triangle) {
                return mix(base, brown, 0.82);
            }
        }
    }

    // Hut (3) — brown square with darker roof
    if (structure_type == 3u) {
        let wall_color = vec4<f32>(0.545, 0.271, 0.075, 1.0); // #8B4513
        let roof_color = vec4<f32>(0.30, 0.14, 0.04, 1.0);
        if (lod == 0u) {
            if (dist < 0.35) {
                return mix(base, wall_color, 0.90);
            }
        } else if (lod == 1u) {
            let fx = cell_frac.x;
            let fy = cell_frac.y;
            if (abs(fx) < 0.30 && abs(fy) < 0.30) {
                // Top third = roof, bottom two-thirds = walls
                if (fy > 0.08) {
                    return mix(base, roof_color, 0.85);
                }
                return mix(base, wall_color, 0.85);
            }
        } else {
            // LOD 2: hut with door cutout
            let fx = cell_frac.x;
            let fy = cell_frac.y;
            if (abs(fx) < 0.30 && abs(fy) < 0.30) {
                // Door: small dark rectangle at bottom center
                let is_door = abs(fx) < 0.07 && fy > -0.30 && fy < -0.10;
                if (is_door) {
                    return mix(base, vec4<f32>(0.10, 0.06, 0.02, 1.0), 0.90);
                }
                if (fy > 0.08) {
                    return mix(base, roof_color, 0.88);
                }
                return mix(base, wall_color, 0.88);
            }
        }
    }

    // Wall (4) — gray stone fills the cell
    if (structure_type == 4u) {
        let stone = vec4<f32>(0.502, 0.502, 0.502, 1.0); // #808080
        if (lod == 0u) {
            return mix(base, stone, 0.90);
        } else if (lod == 1u) {
            return mix(base, stone, 0.88);
        } else {
            // LOD 2: stone with mortar lines
            let fx = fract(world_pos.x * 2.0);
            let fy = fract(world_pos.y * 2.0);
            let mortar = (fx < 0.07 || fy < 0.07);
            if (mortar) {
                return mix(base, vec4<f32>(0.35, 0.35, 0.35, 1.0), 0.80);
            }
            return mix(base, stone, 0.88);
        }
    }

    // ResourceCache (5) — small golden dot (stored goods)
    if (structure_type == 5u) {
        let gold = vec4<f32>(1.0, 0.843, 0.0, 1.0); // #FFD700
        if (lod == 0u) {
            if (dist < 0.30) {
                return mix(base, gold, 0.85);
            }
        } else if (lod == 1u) {
            if (dist < 0.20) {
                return mix(base, gold, 0.88);
            }
            if (dist < 0.28) {
                return mix(base, vec4<f32>(0.70, 0.55, 0.10, 1.0), 0.40);
            }
        } else {
            // LOD 2: golden chest outline
            let fx = cell_frac.x;
            let fy = cell_frac.y;
            if (abs(fx) < 0.22 && abs(fy) < 0.18) {
                let on_edge = abs(fx) > 0.17 || abs(fy) > 0.13;
                if (on_edge) {
                    return mix(base, vec4<f32>(0.60, 0.45, 0.05, 1.0), 0.90);
                }
                return mix(base, gold, 0.88);
            }
        }
    }

    // DirtPath (6) — worn earth trail
    if (structure_type == 6u) {
        let dirt = vec4<f32>(0.62, 0.49, 0.32, 1.0);
        return mix(base, dirt, 0.70);
    }

    // StoneRoad (7) — cobblestone road
    if (structure_type == 7u) {
        let stone_road = vec4<f32>(0.55, 0.55, 0.53, 1.0);
        let cell_frac2 = fract(world_pos);
        let grid = step(0.08, fract(cell_frac2.x * 3.0)) * step(0.08, fract(cell_frac2.y * 3.0));
        let cobble = mix(vec4<f32>(0.45, 0.45, 0.43, 1.0), stone_road, grid);
        return mix(base, cobble, 0.80);
    }

    // SignalBeacon (8) — glowing crystal obelisk
    if (structure_type == 8u) {
        let beacon_blue = vec4<f32>(0.30, 0.70, 1.0, 1.0);
        let glow = 0.85 + sin(time * 2.5 + world_pos.x + world_pos.y) * 0.15;
        let beacon_frac = fract(world_pos) - vec2<f32>(0.5, 0.5);
        if (abs(beacon_frac.x) < 0.08 && beacon_frac.y > -0.25 && beacon_frac.y < 0.30) {
            return mix(base, beacon_blue * glow, 0.92);
        }
        let beacon_dist = length(beacon_frac);
        if (beacon_dist < 0.40) {
            let aura = (1.0 - beacon_dist / 0.40) * 0.30 * glow;
            return mix(base, beacon_blue, aura);
        }
    }

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

    let base_color = biome_base_color(biome_id);
    let structure_id = u32(in.structure_type + 0.5);

    // zoom_blend: 0.0=LOD0, 1.0=LOD1, 2.0=LOD2; fractional = smooth blend between adjacent LODs
    let zoom = clamp(camera.zoom_blend, 0.0, 2.0);

    // ── LOD 0: Macro zoom — flat solid biome color ────────────────────────
    var color_lod0 = base_color;
    if (is_water) {
        color_lod0 = water_depth_color(in.elevation);
    }

    // ── LOD 1: Medium zoom — base + per-cell brightness + signal tinting ──
    let variation = cell_brightness(in.world_pos);
    let d = clamp(water_time.signal_danger,  0.0, 1.0);
    let c = clamp(water_time.signal_comfort, 0.0, 1.0);
    let g = clamp(water_time.signal_grief,   0.0, 1.0);
    var color_lod1: vec4<f32>;
    if (is_water) {
        let depth_color = water_depth_color(in.elevation);
        let pulse = sin(t * 2.0 + in.world_pos.x * 0.5) * 0.03;
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
        // Use LOD 0 structure detail in first half, LOD 1 in second half
        let struct_lod = select(1u, 0u, zoom < 0.5);
        let with_struct = apply_structure(blended, structure_id, in.build_progress, in.world_pos, t, struct_lod);
        return apply_illumination(with_struct, illumination, comfort);
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
            let sway = sin(t * 1.5 + in.world_pos.x * 0.5) * 0.05;
            let canopy_darkness = 0.15 + (sin(in.world_pos.x * 3.0 + t + sway) * cos(in.world_pos.y * 3.0)) * 0.05;
            color_lod2 = mix(color_lod2, vec4<f32>(0.1, 0.3, 0.1, 1.0), canopy_darkness);
        }
    }

    // Blend LOD 1 → LOD 2 when zoom in [1, 2]
    let blend_12 = zoom - 1.0;
    let blended = mix(color_lod1, color_lod2, blend_12);
    let struct_lod = select(1u, 2u, blend_12 >= 0.5);
    let with_struct = apply_structure(blended, structure_id, in.build_progress, in.world_pos, t, struct_lod);
    return apply_illumination(with_struct, illumination, comfort);
}
