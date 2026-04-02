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

// group 2: water time + signal tint + day/night illumination uniform
// Layout: vec4 [time, signal_danger, signal_comfort, signal_grief]
//       + vec4 [illumination, _pad1, _pad2, _pad3]
struct WaterTime {
    time:           f32,
    signal_danger:  f32,
    signal_comfort: f32,
    signal_grief:   f32,
    illumination:   f32, // day/night: 0.0 = full night, 1.0 = full day
    _wt_pad1:       f32,
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
    @location(7) cell_scale:     f32,       // LOD stride — quad covers stride×stride cells (1.0 at full res)
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) atlas_uv:       vec2<f32>,
    @location(1) flags:          f32,
    @location(2) world_pos:      vec2<f32>, // integer cell coords
    @location(3) structure_type: f32,
    @location(4) elevation:      f32,       // terrain elevation [0.0, 1.0]
};

@vertex
fn vs_main(vertex: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // Scale quad to cover stride×stride cells at LOD — eliminates gaps in sparse grid.
    let scale = max(inst.cell_scale, 1.0);
    let world = vec2<f32>(
        inst.world_pos.x + vertex.position.x * scale,
        inst.world_pos.y + vertex.position.y * scale,
    );

    out.clip_position = camera.view_proj * vec4<f32>(world, 0.0, 1.0);

    // Map the quad's 0-1 UV to the specific tile region in the atlas
    // Apply 0.5px inset (0.0005 in UV space for 32px tile in 1024px atlas) to prevent bleeding
    let tile_size = 1.0 / 32.0; // 32px / 1024px atlas
    let inset = 0.0005; // ~0.5px inset at 1024px atlas
    let uv_min = inst.tile_uv + vec2<f32>(inset, inset);
    let uv_range = tile_size - 2.0 * inset;
    out.atlas_uv = uv_min + vertex.uv * uv_range;
    out.flags = inst.flags;
    out.world_pos = inst.world_pos; // pass cell origin (integer coords)
    out.structure_type = inst.structure_type;
    out.elevation = inst.elevation;

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

// Overlay a structure visual on top of the terrain color.
// Returns the modified color. Works at all LOD levels.
fn apply_structure(base: vec4<f32>, structure_type: u32, world_pos: vec2<f32>, time: f32, lod: u32) -> vec4<f32> {
    if (structure_type == 0u) {
        return base;
    }

    let cell_frac = fract(world_pos) - vec2<f32>(0.5, 0.5); // [-0.5, 0.5]
    let dist = length(cell_frac);

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
    let is_water = (biome_id == 1u);
    let illumination = clamp(water_time.illumination, 0.0, 1.0);
    let comfort = clamp(water_time.signal_comfort, 0.0, 1.0);

    let base_color = biome_base_color(biome_id);

    let structure_id = u32(in.structure_type + 0.5);

    // ── LOD 0: Macro zoom — flat solid biome color only ───────────────────
    if (camera.zoom_level == 0u) {
        var color0 = base_color;
        if (is_water) {
            color0 = water_depth_color(in.elevation);
        }
        color0 = apply_structure(color0, structure_id, in.world_pos, t, 0u);
        return apply_illumination(color0, illumination, comfort);
    }

    // ── LOD 1: Medium zoom — base + subtle per-cell variation ─────────────
    if (camera.zoom_level == 1u) {
        let variation = cell_brightness(in.world_pos);
        var color = vec4<f32>(base_color.rgb * variation, 1.0);

        if (is_water) {
            // Depth-based color modulated by animated pulse
            let depth_color = water_depth_color(in.elevation);
            let pulse = sin(t * 2.0 + in.world_pos.x * 0.5) * 0.03;
            color = vec4<f32>(
                clamp(depth_color.r + pulse,        0.0, 1.0),
                clamp(depth_color.g + pulse,        0.0, 1.0),
                clamp(depth_color.b + pulse * 1.5,  0.0, 1.0),
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
            // Snow: white sparkle dot
            if (biome_id == 6u && h < 0.25 && dist < 0.12) {
                color = mix(color, vec4<f32>(1.0, 1.0, 1.0, 1.0), 0.65);
            }
        }
        color = apply_structure(color, structure_id, in.world_pos, t, 1u);
        return apply_illumination(color, illumination, comfort);
    }

    // ── LOD 2: Close zoom — full detail with atlas overlay ────────────────
    let variation = cell_brightness(in.world_pos);
    var color = vec4<f32>(base_color.rgb * variation, 1.0);

    if (is_water) {
        // Depth-based color + wave animation — NO atlas sampling
        let depth_color = water_depth_color(in.elevation);
        let pulse = sin(t * 2.0) * 0.05;
        color = vec4<f32>(
            clamp(depth_color.r + pulse,       0.0, 1.0),
            clamp(depth_color.g + pulse,       0.0, 1.0),
            clamp(depth_color.b + pulse * 1.5, 0.0, 1.0),
            1.0,
        );
        // Shore foam: animated frothy white at shallow edge (elevation > 0.22)
        if (in.elevation > 0.22) {
            let foam_t = clamp((in.elevation - 0.22) / 0.06, 0.0, 1.0);
            let foam_anim = sin(t * 3.0 + in.world_pos.x * 2.5 + in.world_pos.y * 1.8) * 0.5 + 0.5;
            let foam_mix = foam_t * (0.5 + foam_anim * 0.5);
            color = mix(color, vec4<f32>(0.92, 0.96, 1.0, 1.0), clamp(foam_mix, 0.0, 0.85));
        }
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

        // Forest: multi-layer canopy (biome_id == 2)
        if (biome_id == 2u && h < 0.40) {
            let offset = vec2<f32>(0.3 + h2 * 0.4, 0.3 + h * 0.4);
            let cell_frac = fract(in.world_pos) - offset;
            let dist = length(cell_frac);
            // Ground shadow
            if (dist < 0.40) {
                color = mix(color, vec4<f32>(0.06, 0.18, 0.03, 1.0), 0.15);
            }
            // Outer canopy
            if (dist < 0.35) {
                let green = 0.38 + h2 * 0.12;
                color = mix(color, vec4<f32>(0.08, green, 0.05, 1.0), 0.80);
            }
            // Inner canopy highlight
            if (dist < 0.18) {
                let light_green = 0.55 + h2 * 0.15;
                color = mix(color, vec4<f32>(0.18, light_green, 0.10, 1.0), 0.55);
            }
            // Trunk center
            if (dist < 0.06) {
                color = mix(color, vec4<f32>(0.36, 0.22, 0.08, 1.0), 0.90);
            }
        }

        // Grassland: varied ground cover (biome_id == 0)
        if (biome_id == 0u) {
            // Tall grass tuft
            if (h < 0.25) {
                let gx = 0.5 + h2 * 0.3 - 0.15;
                let cell_frac_g = fract(in.world_pos) - vec2<f32>(gx, 0.5);
                if (abs(cell_frac_g.x) < 0.04 && cell_frac_g.y > -0.25 && cell_frac_g.y < 0.20) {
                    color = mix(color, vec4<f32>(0.28, 0.52, 0.08, 1.0), 0.72);
                }
            }
            // Small flower (h in 0.70-0.78)
            if (h > 0.70 && h < 0.78) {
                let cell_frac_f = fract(in.world_pos) - vec2<f32>(0.5 + h2 * 0.3 - 0.15, 0.5 + h * 0.2 - 0.1);
                let dist_f = length(cell_frac_f);
                if (dist_f < 0.10) {
                    // Pink/yellow/white based on h2
                    var flower_color: vec4<f32>;
                    if (h2 < 0.33) {
                        flower_color = vec4<f32>(0.98, 0.60, 0.70, 1.0); // pink
                    } else if (h2 < 0.66) {
                        flower_color = vec4<f32>(0.98, 0.90, 0.30, 1.0); // yellow
                    } else {
                        flower_color = vec4<f32>(0.95, 0.95, 0.95, 1.0); // white
                    }
                    color = mix(color, flower_color, 0.80);
                }
            }
            // Berry bush (h > 0.90)
            if (h > 0.90) {
                let cell_frac_b = fract(in.world_pos) - vec2<f32>(0.5 + h2 * 0.2 - 0.1, 0.5);
                let dist_b = length(cell_frac_b);
                if (dist_b < 0.20) {
                    color = mix(color, vec4<f32>(0.12, 0.30, 0.06, 1.0), 0.75); // dark green bush
                }
                // Red berries: small dots offset from center
                let berry1 = fract(in.world_pos) - vec2<f32>(0.5 + h2 * 0.2 - 0.1 + 0.08, 0.5 + 0.06);
                let berry2 = fract(in.world_pos) - vec2<f32>(0.5 + h2 * 0.2 - 0.1 - 0.07, 0.5 - 0.05);
                if (length(berry1) < 0.05 || length(berry2) < 0.05) {
                    color = mix(color, vec4<f32>(0.85, 0.10, 0.10, 1.0), 0.90);
                }
            }
        }

        // Wetland: reed tuft (dark yellow-green vertical smear)
        if (biome_id == 5u && h < 0.20) {
            let cell_frac = fract(in.world_pos) - vec2<f32>(0.5 + h2 * 0.2 - 0.1, 0.5);
            if (abs(cell_frac.x) < 0.05 && cell_frac.y > -0.22 && cell_frac.y < 0.22) {
                color = mix(color, vec4<f32>(0.30, 0.42, 0.08, 1.0), 0.75);
            }
        }

        // Desert: dune shading + dead shrub + cactus (biome_id == 3)
        if (biome_id == 3u) {
            // Dune wave shading
            let dune_wave = sin(in.world_pos.x * 0.8 + in.world_pos.y * 0.3) * 0.03;
            color = vec4<f32>(
                clamp(color.r + dune_wave, 0.0, 1.0),
                clamp(color.g + dune_wave * 0.8, 0.0, 1.0),
                clamp(color.b, 0.0, 1.0),
                1.0,
            );
            // Dead shrub (h in 0.85-0.92)
            if (h > 0.85 && h < 0.92) {
                let sx = 0.5 + h2 * 0.2 - 0.1;
                let shrub_frac = fract(in.world_pos) - vec2<f32>(sx, 0.5);
                // Vertical stick
                if (abs(shrub_frac.x) < 0.03 && shrub_frac.y > -0.18 && shrub_frac.y < 0.15) {
                    color = mix(color, vec4<f32>(0.42, 0.28, 0.10, 1.0), 0.80);
                }
                // Branch dots
                let b1 = fract(in.world_pos) - vec2<f32>(sx + 0.08, 0.5 + 0.05);
                let b2 = fract(in.world_pos) - vec2<f32>(sx - 0.09, 0.5 + 0.02);
                let b3 = fract(in.world_pos) - vec2<f32>(sx + 0.05, 0.5 - 0.06);
                if (length(b1) < 0.03 || length(b2) < 0.03 || length(b3) < 0.03) {
                    color = mix(color, vec4<f32>(0.42, 0.28, 0.10, 1.0), 0.80);
                }
            }
            // Cactus — vertical green bar with arms
            if (h < 0.08) {
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
        }

        // Mountain: boulders + snow cap + small rubble (biome_id == 4)
        if (biome_id == 4u) {
            // Large boulder (h < 0.15)
            if (h < 0.15) {
                let cell_frac = fract(in.world_pos) - vec2<f32>(0.5, 0.5);
                let dist = length(cell_frac);
                if (dist < 0.25) {
                    let grey = 0.42 + h2 * 0.18;
                    color = mix(color, vec4<f32>(grey, grey, grey * 0.95, 1.0), 0.75);
                    // Snow cap if elevation > 0.8
                    if (in.elevation > 0.8 && cell_frac.y > 0.06) {
                        color = mix(color, vec4<f32>(0.93, 0.95, 1.0, 1.0), 0.80);
                    }
                }
            }
            // Small rubble (h in 0.50-0.70)
            if (h > 0.50 && h < 0.70) {
                let rx = 0.5 + h2 * 0.5 - 0.25;
                let ry = 0.5 + fract(h * 7.3) * 0.5 - 0.25;
                let rubble_frac = fract(in.world_pos) - vec2<f32>(rx, ry);
                if (length(rubble_frac) < 0.06) {
                    let rg = 0.38 + h2 * 0.20;
                    color = mix(color, vec4<f32>(rg, rg, rg * 0.92, 1.0), 0.65);
                }
            }
        }

        // Snow: snowflake dot (near-white sparkle)
        if (biome_id == 6u && h < 0.18) {
            let cell_frac = fract(in.world_pos) - vec2<f32>(0.5, 0.5);
            let dist = length(cell_frac);
            if (dist < 0.12) {
                color = mix(color, vec4<f32>(1.0, 1.0, 1.0, 1.0), 0.70);
            }
        }
    }

    color = apply_structure(color, structure_id, in.world_pos, t, 2u);
    return apply_illumination(color, illumination, comfort);
}
