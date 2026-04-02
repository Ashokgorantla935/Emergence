# Wave 15: WorldBox Aesthetic Revamp — Terrain Engine + Entity Compositor + Infrastructure

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**
**Prerequisite: Wave 14 stability fixes must be applied first**
**Target: 190/100 WorldBox fidelity**

---

## Context: What We Have Today

The simulation currently renders:
- **Terrain:** Procedural biome colors via `terrain.wgsl` fragment shader (no atlas sampling — purely procedural shapes for trees, cacti, rocks, flowers, reeds). Biomes are: Grassland(0), Water(1), Forest(2), Desert(3), Mountain(4), Wetland(5), Snow(6), Tundra(7).
- **Beings:** `BeingRenderer` samples from a 4x4 entity spritesheet (Sprout Lands `Basic Charakter Spritesheet.png` — 192x192px, 48x48 frames). The `entity_bind_group` is already loaded in `state.rs` (lines 141-210) and bound at slot 1 for the sprite pipeline.
- **Structures:** Rendered procedurally in `terrain.wgsl` via `apply_structure()` — campfire, lean-to, hut, wall, resource cache are all SDF shapes.
- **Objects:** `ObjectRenderer` is **completely disabled** (`if false` wrapper at line 2403 of main.rs).

### Available Asset Packs (on disk)
```
assets/sprites/packs/Sprout Lands - Sprites - Basic pack/.../
  Characters/
    Basic Charakter Spritesheet.png    — 192x192, 4 cols × 4 rows of 48x48 human frames
    Basic Charakter Actions.png        — 96x576, 2 cols × 12 rows of 48x48 action frames
    Tools.png                          — 128x96, tool item sprites (axes, hammers, pickaxes, etc.)
    Free Chicken Sprites.png           — chicken animation frames
    Free Cow Sprites.png               — cow animation frames
  Objects/
    Paths.png                          — 64x48, dirt path tile variants (4 connection types)
    Basic Furniture.png                — furniture sprites
    Basic tools and meterials.png      — tools + stone/wood materials
    Wooden House.png                   — wooden house parts (walls, door, roof)
    Fences.png                         — fence tile variants
    Basic Grass Biom things 1.png      — grass flowers, mushrooms, bushes
    Basic Plants.png                   — trees, crops at growth stages
    Chest.png                          — animated chest
    Wood Bridge.png                    — bridge tiles
  Tilesets/
    Grass.png                          — 16x16 grass autotile sheet
    Water.png                          — animated water tiles
    Hills.png                          — elevation transition tiles
    Wooden_House_Walls_Tilset.png      — wall autotile
    Wooden_House_Roof_Tilset.png       — roof autotile
    Fences.png                         — fence autotile
    Tilled_Dirt.png                    — farm soil tiles
    Doors.png                          — door sprites
```

---

## Part A: Enhanced Procedural Terrain (terrain.wgsl)

**Goal:** Make the world feel alive at close zoom. Currently at LOD 2, biomes are flat colored blobs with tiny procedural dots. WorldBox has lush, varied terrain that feels hand-painted.

### A1. Enhance the LOD 2 Decoration Layer

**File: `crates/emergence-viewer/src/renderer/shaders/terrain.wgsl`**

The existing LOD 2 decorations (lines 410-486) are primitive. Upgrade them:

#### Forest biome (biome_id == 2): Multi-layer canopy
Current: Single blob with trunk center.
New: 2 decoration layers per cell — a shadow circle underneath, then a multi-tone canopy on top. Use `h3 = fract(sin(dot(cell, vec2(41.123, 91.47))) * 43758.5453)` for a third hash.

```wgsl
// Forest: layered tree with shadow
if (biome_id == 2u && h < 0.40) {
    let offset = vec2<f32>(0.3 + h2 * 0.4, 0.3 + h * 0.4);
    let cell_frac = fract(in.world_pos) - offset;
    let dist = length(cell_frac);
    
    // Ground shadow (slightly larger, dark)
    if (dist < 0.40) {
        color = mix(color, vec4<f32>(0.05, 0.15, 0.02, 1.0), 0.15);
    }
    // Outer canopy (medium green)
    if (dist < 0.35) {
        let green = 0.30 + h2 * 0.20;
        color = mix(color, vec4<f32>(0.08, green, 0.04, 1.0), 0.80);
    }
    // Inner canopy highlight (lighter)
    if (dist < 0.18) {
        let green = 0.40 + h2 * 0.15;
        color = mix(color, vec4<f32>(0.15, green, 0.08, 1.0), 0.55);
    }
    // Trunk
    if (dist < 0.06) {
        color = mix(color, vec4<f32>(0.35, 0.22, 0.10, 1.0), 0.90);
    }
}
```

#### Grassland biome (biome_id == 0): Varied ground cover
Current: Single flower dot at 12% density.
New: 3 layers — tall grass tufts (25%), small flowers (8%), berry bushes (4%).

```wgsl
// Grassland: multi-element ground cover
if (biome_id == 0u) {
    let cell_frac = fract(in.world_pos) - vec2<f32>(0.5, 0.5);
    let dist = length(cell_frac);
    
    // Tall grass tuft
    if (h < 0.25 && abs(cell_frac.x) < 0.08 && cell_frac.y > -0.15 && cell_frac.y < 0.20) {
        let grass_g = 0.55 + h2 * 0.25;
        color = mix(color, vec4<f32>(0.35, grass_g, 0.15, 1.0), 0.50);
    }
    // Small flower
    if (h > 0.70 && h < 0.78 && dist < 0.10) {
        // Flower color varies: pink, yellow, white based on h2
        var flower_color = vec4<f32>(0.95, 0.45, 0.55, 1.0); // pink
        if (h2 > 0.66) { flower_color = vec4<f32>(1.0, 0.90, 0.20, 1.0); } // yellow
        else if (h2 > 0.33) { flower_color = vec4<f32>(0.95, 0.95, 1.0, 1.0); } // white
        color = mix(color, flower_color, 0.70);
    }
    // Berry bush
    if (h > 0.90 && dist < 0.20) {
        color = mix(color, vec4<f32>(0.15, 0.45, 0.10, 1.0), 0.65); // bush body
        if (dist < 0.08) {
            color = mix(color, vec4<f32>(0.85, 0.15, 0.20, 1.0), 0.70); // red berries
        }
    }
}
```

#### Mountain biome (biome_id == 4): Layered rocks + snow caps
```wgsl
// Mountain: multi-sized boulders + snow dusting
if (biome_id == 4u) {
    let cell_frac = fract(in.world_pos) - vec2<f32>(0.5, 0.5);
    let dist = length(cell_frac);
    
    // Large boulder (15% of cells)
    if (h < 0.15 && dist < 0.25) {
        let grey = 0.35 + h2 * 0.20;
        color = mix(color, vec4<f32>(grey, grey, grey * 0.95, 1.0), 0.70);
        // Snow cap on tall mountains (elevation > 0.8)
        if (in.elevation > 0.80 && cell_frac.y > 0.05) {
            color = mix(color, vec4<f32>(0.95, 0.95, 0.98, 1.0), 0.65);
        }
    }
    // Small rubble (additional 20% of cells)
    let offset2 = vec2<f32>(h2 * 0.6, h * 0.4);
    let cell_frac2 = fract(in.world_pos) - offset2;
    let dist2 = length(cell_frac2);
    if (h > 0.50 && h < 0.70 && dist2 < 0.10) {
        let grey = 0.45 + h2 * 0.15;
        color = mix(color, vec4<f32>(grey, grey, grey, 1.0), 0.50);
    }
}
```

#### Desert biome (biome_id == 3): Sand dune shading + sparse vegetation
```wgsl
// Desert: dune shading + improved cactus + dead shrub
if (biome_id == 3u) {
    let cell_frac = fract(in.world_pos) - vec2<f32>(0.5, 0.5);
    
    // Subtle dune shading (wave pattern based on world position)
    let dune_wave = sin(in.world_pos.x * 0.8 + in.world_pos.y * 0.3) * 0.03;
    color = vec4<f32>(
        clamp(color.r + dune_wave, 0.0, 1.0),
        clamp(color.g + dune_wave * 0.7, 0.0, 1.0),
        clamp(color.b + dune_wave * 0.3, 0.0, 1.0),
        1.0
    );
    
    // Cactus (keep existing, already good)
    // Dead shrub
    if (h > 0.85 && h < 0.92) {
        if (abs(cell_frac.x) < 0.04 && cell_frac.y > -0.12 && cell_frac.y < 0.08) {
            color = mix(color, vec4<f32>(0.50, 0.38, 0.20, 1.0), 0.60);
        }
        // Branches
        let bx = cell_frac.x;
        let by = cell_frac.y;
        if (abs(bx - 0.06) < 0.03 && abs(by - 0.02) < 0.03) {
            color = mix(color, vec4<f32>(0.50, 0.38, 0.20, 1.0), 0.50);
        }
        if (abs(bx + 0.05) < 0.03 && abs(by + 0.04) < 0.03) {
            color = mix(color, vec4<f32>(0.50, 0.38, 0.20, 1.0), 0.50);
        }
    }
}
```

#### Water: Shore foam line
Add a beach/shore transition at the boundary between water and land. In the water depth color function or LOD 2 water section:
```wgsl
// Shore foam: white froth where water meets land (elevation near threshold)
if (is_water && in.elevation > 0.22) {
    let foam_t = clamp((in.elevation - 0.22) / 0.06, 0.0, 1.0);
    let foam_wave = sin(t * 3.0 + in.world_pos.x * 2.0 + in.world_pos.y * 1.5) * 0.3 + 0.7;
    color = mix(color, vec4<f32>(0.92, 0.95, 1.0, 1.0), foam_t * foam_wave * 0.5);
}
```

---

## Part B: Enhanced Character Rendering

**Goal:** Characters should look like distinct people, not identical kittens.

### B1. Entity Spritesheet Architecture

The current `entity_bind_group` in `state.rs` already loads `Basic Charakter Spritesheet.png` (192x192, 4x4 grid of 48x48 frames). The `being_renderer.rs` uses `ENTITY_CELL = 1.0 / 4.0` for humans.

**Current frame layout (4x4 grid):**
- Row 0: Down-facing (idle, walk1, walk2, walk3)
- Row 1: Up-facing (idle, walk1, walk2, walk3)  
- Row 2: Left-facing (idle, walk1, walk2, walk3)
- Row 3: Right-facing (idle, walk1, walk2, walk3)

The `animation.rs` `atlas_uv()` should map being direction + walk frame to the correct cell. Verify this mapping is correct. Currently `beings.rs` line 120-123 already branches on `is_human` to use `ENTITY_CELL` vs `ATLAS_CELL`.

### B2. Skin Tone + Clothing Tinting

The being shader (`being_sprite.wgsl`) already receives `skin_tone` (3 floats) and `emotion_tint` (3 floats) per instance. The shader should:

1. Sample the entity texture
2. Detect "skin" pixels (pixels that are close to the base sprite's default skin color — typically a yellow/peach tone, around RGB(255, 213, 163))
3. Tint those pixels toward the being's `skin_tone`
4. Detect "clothing" pixels (typically the purple/blue outfit pixels)
5. Tint those toward `emotion_tint` (dominant emotion color)

**File: `crates/emergence-viewer/src/renderer/shaders/being_sprite.wgsl`**

Add after sampling:
```wgsl
// Skin detection: if pixel is close to default skin color, tint toward being's skin_tone
let default_skin = vec3<f32>(1.0, 0.835, 0.639); // Base spritesheet skin
let skin_dist = length(texel.rgb - default_skin);
if (skin_dist < 0.25) {
    let blend = 1.0 - (skin_dist / 0.25);
    texel = vec4<f32>(mix(texel.rgb, skin_tone, blend * 0.6), texel.a);
}

// Clothing detection: if pixel is close to default outfit color (purple/blue), tint to emotion
let default_outfit = vec3<f32>(0.545, 0.388, 0.757); // Base purple outfit
let outfit_dist = length(texel.rgb - default_outfit);
if (outfit_dist < 0.30) {
    let blend = 1.0 - (outfit_dist / 0.30);
    texel = vec4<f32>(mix(texel.rgb, emotion_tint, blend * 0.5), texel.a);
}
```

This means every being will have a visually unique appearance based on their personality hash (skin) and current emotional state (clothing color).

---

## Part C: Infrastructure — Roads & Progressive Buildings

### C1. Road System (Engine Side)

**File: `crates/emergence-core/src/world/terrain.rs`**

Add road types to the structure system. Currently structures are stored as `structure: Vec<u8>` (per-cell). The existing types are:
- 0 = None
- 1 = Campfire
- 2 = LeanTo
- 3 = Hut
- 4 = Wall
- 5 = ResourceCache

**Add:**
- 6 = `DirtPath` — naturally forms when beings walk the same route repeatedly
- 7 = `StoneRoad` — constructed by beings with stone resources

**Movement cost integration:**
In the movement/pathfinding logic (`tick.rs` or `actions.rs`), when calculating movement for a being:
```rust
let structure = world.terrain.structure[cell_idx];
let move_multiplier = match structure {
    6 => 0.5,  // DirtPath: 2x speed
    7 => 0.3,  // StoneRoad: 3.3x speed
    _ => 1.0,  // Normal terrain
};
```

**Dynamic path formation:**
Add a `trample: Vec<u8>` array to terrain. Each time a being moves through a cell:
```rust
terrain.trample[cell_idx] = terrain.trample[cell_idx].saturating_add(1);
if terrain.trample[cell_idx] > 200 && terrain.structure[cell_idx] == 0 {
    terrain.structure[cell_idx] = 6; // Auto-create DirtPath
    terrain.trample[cell_idx] = 0;
}
```

### C2. Road Rendering (Shader Side)

**File: `crates/emergence-viewer/src/renderer/shaders/terrain.wgsl`**

Add to `apply_structure()`:
```wgsl
// DirtPath (6) — brown beaten earth
if (structure_type == 6u) {
    let dirt = vec4<f32>(0.62, 0.49, 0.32, 1.0); // #9E7D52
    return mix(base, dirt, 0.70);
}

// StoneRoad (7) — grey cobblestone with subtle pattern
if (structure_type == 7u) {
    let stone = vec4<f32>(0.55, 0.55, 0.53, 1.0);
    let cell_frac = fract(in.world_pos);
    // Cobblestone grid pattern
    let grid = step(0.08, fract(cell_frac.x * 3.0)) * step(0.08, fract(cell_frac.y * 3.0));
    let cobble = mix(vec4<f32>(0.45, 0.45, 0.43, 1.0), stone, grid);
    return mix(base, cobble, 0.80);
}
```

### C3. Progressive Building Construction

**File: `crates/emergence-core/src/world/terrain.rs`**

Add `build_progress: Vec<u8>` array to `Terrain`. Range 0-255 where:
- 0-84: Stage 0 (Scaffold)
- 85-169: Stage 1 (Frame)
- 170-255: Stage 2 (Finished)

When a being constructs a building, increment `build_progress[cell]` each tick they work.

**Shader Side** (`terrain.wgsl`): Pass `build_progress / 255.0` as a float via the instance data. Then in `apply_structure()`:

For Hut (structure_type == 3):
```wgsl
if (structure_type == 3u) {
    let progress = in.build_progress; // 0.0 to 1.0
    
    if (progress < 0.33) {
        // Stage 0: Scaffold — translucent wooden frame outline
        let fx = cell_frac.x;
        let fy = cell_frac.y;
        let on_frame = abs(abs(fx) - 0.28) < 0.03 || abs(abs(fy) - 0.28) < 0.03;
        if (on_frame) {
            return mix(base, vec4<f32>(0.55, 0.35, 0.15, 1.0), 0.50);
        }
    } else if (progress < 0.66) {
        // Stage 1: Frame — solid walls appearing, no roof yet
        let fx = cell_frac.x;
        let fy = cell_frac.y;
        if (abs(fx) < 0.28 && abs(fy) < 0.28) {
            return mix(base, wall_color, 0.65);
        }
    } else {
        // Stage 2: Finished (existing full hut logic)
        // ... keep existing hut rendering
    }
}
```

### C4. Signal Beacon (Novel Innovation)

**Engine: `terrain.rs`**
Add structure type 8 = `SignalBeacon`.

**Effect in `signal.rs`**: Each tick, for each `SignalBeacon` cell:
```rust
// Signal Beacon forces Comfort channel to max in a 10-cell radius
if terrain.structure[idx] == 8 {
    let bx = (idx % w) as i32;
    let by = (idx / w) as i32;
    for dy in -10..=10 {
        for dx in -10..=10 {
            let nx = (bx + dx).clamp(0, w as i32 - 1) as usize;
            let ny = (by + dy).clamp(0, h as i32 - 1) as usize;
            let r = ((dx*dx + dy*dy) as f32).sqrt();
            if r <= 10.0 {
                let ni = ny * w + nx;
                let boost = 5.0 * (1.0 - r / 10.0); // Linear falloff
                channels[COMFORT][ni] = (channels[COMFORT][ni] + boost).min(10.0);
            }
        }
    }
}
```

**Shader rendering** (add to `apply_structure()`):
```wgsl
// SignalBeacon (8) — glowing crystal obelisk
if (structure_type == 8u) {
    let beacon_blue = vec4<f32>(0.30, 0.70, 1.0, 1.0);
    let glow = 0.85 + sin(time * 2.5 + world_pos.x + world_pos.y) * 0.15;
    let cell_frac = fract(in.world_pos) - vec2<f32>(0.5, 0.5);
    
    // Obelisk body
    if (abs(cell_frac.x) < 0.08 && cell_frac.y > -0.25 && cell_frac.y < 0.30) {
        return mix(base, beacon_blue * glow, 0.92);
    }
    // Glow aura
    let dist = length(cell_frac);
    if (dist < 0.40) {
        let aura = (1.0 - dist / 0.40) * 0.30 * glow;
        return mix(base, beacon_blue, aura);
    }
}
```

---

## Part D: Instance Data Update for build_progress

To pass `build_progress` to the shader, the terrain instance layout needs a new field.

**File: `crates/emergence-viewer/src/renderer/terrain.rs`**

Currently the `TerrainInstance` struct has fields: `world_pos`, `tile_uv`, `flags`, `elevation`, `structure_type`. 

**Add `build_progress: f32`** — this increases the instance stride. Update the vertex buffer layout in `state.rs` to add one more `Float32` attribute.

Then in `rebuild_instances_viewport`, read from `terrain.build_progress[idx]` and divide by 255.0.

---

## Execution Order

1. **Part A first** — pure shader changes, zero compilation risk, immediate visual impact
2. **Part B next** — shader + minor `beings.rs` verification  
3. **Part C last** — requires engine changes (terrain.rs) + shader + instance layout changes

## Verification

After each part, compile and run:
```bash
cargo run -p emergence-app -- --autostart
```

- **Part A:** Zoom in to close view. Forest should have layered canopy trees with shadows. Grassland should have varied flowers and grass tufts. Mountains should have multi-sized boulders. Water shores should have foam.
- **Part B:** Beings should have visually distinct skin tones and emotion-colored clothing.  
- **Part C:** Build a hut (god tool or being construction). Watch it progress through scaffold → frame → finished stages. Walk beings back and forth to see dirt paths form.
