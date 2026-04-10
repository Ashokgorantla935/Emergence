# V54: Master Integration Protocol - Macroscopic Scale, Spatial Audio, & Deep LOD

## Author: God Architect (Antigravity/Gemini)
## Target: Staff Engineer (Claude)

Claude, do not skim this document. Do not auto-generate fallbacks. Do not guess on grid sizes or architecture integration. The Visionary has demanded a strictly stitched 190/100 world. You must implement these exact structs, mathematics, and shader logics.

---

## 1. Absolute Asset Ground Truth & Splicing
Our 1024x1024 pixel sheets require perfect floating-point UV offset math. Do not slice using `u32` integers natively, or you will experience 4-pixel drift at the edges.

**1.1 Grid Dimensions Matrix (HARDCODED):**
You must map these EXACT values into the `icon_loader.rs` sheet slicer or the WGSL shader configurations:
- `human_races_190.png` -> **16x12 Grid** (16 columns, 12 rows)
- `flora_spritesheet_190.png` -> **16x12 Grid**
- `terrain_spritesheet_190.png` -> **16x16 Grid**
- `fauna_and_races_spritesheet_190.png` -> **12x12 Grid**
- `architecture_spritesheet_190.png` -> **8x8 Grid**
- `minerals_spritesheet_190.png` -> **8x8 Grid**
- `exotic_biomes_spritesheet_190.png` -> **8x8 Grid**
- `consumables_spritesheet_190.png` -> **10x12 Grid**
- `powers_ui_spritesheet_190.png` -> **10x10 Grid**
- `vfx_and_traits_spritesheet_190.png` -> **10x10 Grid**

**1.2 The Float UV Splicing Math (WGSL / Loaders):**
If rendering via single material atlas in WGSL, the UV mapping formula MUST strictly be:
```wgsl
let cell_width = 1.0 / f32(columns); // e.g. 1.0 / 16.0
let cell_height = 1.0 / f32(rows);   // e.g. 1.0 / 12.0
let col = f32(instance.atlas_index % columns);
let row = f32(instance.atlas_index / columns);

let final_uv = vec2<f32>(
    (in.uv.x * cell_width) + (col * cell_width),
    (in.uv.y * cell_height) + (row * cell_height)
);
```

---

## 2. Spatial Audio Engine (Zoom Dynamic)
A silent micro-world feels lifeless. We must stitch entity sounds based on the camera's zoom frustum.

**2.1 Rust Audio Emitter State (`audio/mod.rs`):**
Implement a spatial audio mixer that samples the entities *currently visible* on screen.
When the `Camera::zoom` parameter identifies that the player is zoomed deeply into the terrain (`zoom < 5.0` base units, or whatever local micro-threshold is), you must calculate local density:
1. Scan `BeingsHot` and `Flora` within the camera's viewport bounds.
2. Tally the counts of: `Humans`, `Fauna (Animals)`, `Water Nodes`, `Trees`.
3. Blend and spawn audio tracks:
   - If `Humans > 0`: Slowly ramp up a town/chatter loop volume relative to human count.
   - If `Fauna > 0`: Trigger intermittent animal sounds (wolves howling, birds chirping) spaced out randomly over `f32` seconds.
   - If `Zoom` is pulled back strictly to Macro (continent view): Mute micro-sounds and ONLY play macro ambiance (e.g., deep wind or cosmic drones).

---

## 3. Decoupled Biological Scale & Prediction Pipeline
Entities can no longer be rendered purely `1 cell = 1x1 sprite`. An apple is tiny, an elder tree is massive. We also must decouple the graphic framerate from the simulation ticks to ensure smooth zooming.

**3.1 WGPU Instance Buffer Struct Update (`emergence-viewer/src/renderer/state.rs`)**
Inject both `velocity` and `scale_multiplier` into every instanced object (beings, flora, buildings):
```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EntityInstance {
    pub position: [f32; 2],         // Current Simulation World Pos
    pub velocity: [f32; 2],         // [dx, dy] per second (For dead-reckoning)
    pub atlas_index: u32,           // Mapped integer index for the UV sheet
    pub scale_multiplier: f32,      // Absolute physical biological size
    pub align_padding: [f32; 2],    // Padding to maintain 16-byte alignment
}
```

**3.2 Rust Packing Rules (CPU Culling and Scaling):**
In `beings.rs` and `objects.rs`:
- Scale Mappings: Fruits/Loot = `0.1`, Fauna = `0.4`, Humans = `0.8`, Trees = `2.0`, Towns = `3.0`.
- **Frustum Culling**: Before pushing data to `queue.write_buffer()`, calculate if the entity is within `camera_bounds`. Do NOT ship entities on the opposite side of the world to the GPU.

**3.3 The Dead-Reckoning & Scale Vertex Transformation (WGSL)**
Inside `terrain.wgsl` or your relevant vertex shader, pull in a Time Uniform (Time since last simulation tick) to interpolate motion seamlessly at 144hz.
```wgsl
struct TimeUniforms {
    time_since_last_tick: f32, // Delta-time purely since the CPU last updated world_pos
    _padding: vec3<f32>,
}
@group(1) @binding(1) var<uniform> time_data: TimeUniforms;

@vertex
fn main(in: VertexInput, instance: EntityInstance) -> VertexOutput {
    // 1. Biological scaling pushes the quad geometry outward
    let bio_scaled_vertex = in.quad_vertex * instance.scale_multiplier;
    
    // 2. Dead-reckoning GPU interpolation calculates micro-smooth movement
    let predicted_pos = instance.position + (instance.velocity * time_data.time_since_last_tick);
    
    // 3. Transform to absolute world space
    let world_pos = bio_scaled_vertex + predicted_pos;
    
    // 4. Transform to camera Clip Space
    var out: VertexOutput;
    out.clip_position = camera.proj_view * vec4<f32>(world_pos.x, world_pos.y, 0.0, 1.0);
    out.uv = in.uv; // Adjusted later by atlas math
    
    return out;
}
```

---

## 4. Continuous Level of Detail (LOD) - The GPU Culling
To prevent the GPU from choking when zoomed all the way out, entities smaller than a screen pixel must be math-erased.

**4.1 WGSL Screen Space Discard:**
Inside your Vertex shader right after calculating `world_pos`:
```wgsl
// BaseCellWidth in screen pixels changes based on Camera Zoom
// If W_screen < 1.0 pixel, DO NOT DRAW IT.
let screen_width_pixels = (BaseCellWidth * instance.scale_multiplier) / camera.zoom;

if (screen_width_pixels < 1.0) {
    // Force coordinates immediately to infinity/discard bounds
    out.clip_position = vec4<f32>(2000.0, 2000.0, 2000.0, 1.0); 
    return out;
}
```
**Macro-Density Fallback:** For objects culled in Step 4.1, their visual weight must be absorbed by the core terrain shader interpolating a low-res *Density Map* (rendering a generic shadow canopy over high-density forest regions without rendering discrete trees).

Claude, integrate these changes across the viewers, instancing buffers, and `audio` hooks. Confirm completion of the mathematical pipelines when finished.

---

## 5. Orbit ⇔ Ground Seamless LOD Transition (The Tabletop Effect)
Entities must dynamically change physical representation based on the user's zoom factor to provide maximum legibility and a tactile, physical feel.

**5.1 MACRO (Orbit View): Flat Heatmap Assets**
When `camera_zoom` is low (orbit scale), entities drop ALL 3D approximations and shadow casting. Instead, they map purely as flat 2D markers (or native sprites) that scale seamlessly on the terrain, resembling paint on the ground.

**5.2 MICRO (Ground View): Parallax 3D Pins**
When the user zooms in closely, the assets undergo a geometry transition:
1. **The Billboarding Hack:** Counter-scale the quad (`scale = 1.0 / zoom`) so the entity stands up and locks to a rigid physical screen pixel size (e.g., 32x48px).
2. **True Perspective Parallax:** Calculate distance from the center of the physical monitor (`dx = screenPosX - centerX`). Keep the drop shadow anchored statically to the 2D world map coordinate. Translate the actual Pin sprite outward along the `(dx, dy)` vectors proportional to the distance from the center of the screen `(pinLeanX = dx * Parallax_Factor)`. This generates the perfect illusion of physical tabletop board-game pieces leaning under camera perspective.

---

## 6. Micro-Fractal Terrain Generation (Infinite Resolution)
To prevent `terrain_spritesheet_190_seamless.png` from pixelating or stretching at extreme close-up (`LOD2`), macro-noise is insufficient.
Inside `terrain.wgsl`'s fragment shader, inject high-frequency mathematical noise (`organic_noise(world_pos * 18.0)`) into the `LOD2` branch. Mask this noise contextually by biome (e.g. mapping vertical micro-oscillations into `color_lod2` to simulate billions of sharp grass blades). This guarantees infinite sub-pixel crispness regardless of zoom proximity.
