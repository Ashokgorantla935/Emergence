# Visual Overhaul Bible: Swarm OS V2

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

Claude, thank you for the meticulous audit. You’ve accurately identified the core issue: we are taking procedural math meant for abstract visualization and improperly forcing photorealistic/pixel-art assets through it. We are going to lock in a strict multi-LOD rendering pipeline, fix the shading bugs, and formalize the atlas. 

Here is your exact specification to reach 190/100 visually.

---

## 1. The Atlas Layout (The Foundation)
Bugs 1, 3, and 4 stem from an ambiguous atlas. We are unifying the 512x512 atlas (assuming 32x32 tiles = 16 rows × 16 cols).
**Strict Layout Contract:**
* **Rows 0-3 (Terrain):** Base tiles. Grass (Row 0), Sand (Row 1), Mountain/Rock (Row 2), Snow/Dirt (Row 3).
* **Rows 4-6 (Decorations):** Trees, bushes, cacti, rocks.
* **Rows 7-8 (Buildings):** Huts, houses, towers.
* **Rows 9-14 (Beings):** 1 row per entity type. E.g., Row 9 = Human. Columns 0-3 = Walk, Columns 4-7 = Work, Columns 8-11 = Fight. 
* **Row 15 (UI & Effects):** Particles, icons, badges.

**Shader Bug Fixes:**
* **Bug 1 (Animation Offset):** Change `atlas_col = ((state as u32) + frame).min(31)` to `atlas_col = state_base_col + (frame % frame_count)`.
* **Bug 4 (Recoloring):** Delete the `is_skin = atlas_color.r > 0.7` logic entirely. We are using real Sunnyside sprites now. Render them as raw `tex_color`. If you need allegiance colors, overlay a multiplying tint (`tex_color * tint`) only on specific UI rows or use a dedicated tint mask.

---

## 2. Multi-LOD Pipeline Specification

We will implement 3 distinct Zoom thresholds based on camera height/visible cells.

### Zoom Level 1: Macro / World View (> 150 cells visible)
*We do not render sprites or textures here to avoid black speckle aliasing.*
* **Terrain:** Pure, flat, WorldBox-palette solid colors in the fragment shader. No `textureSample`.
  * Deep ocean: `#1E3A8A`, Shallows: `#3B82F6`
  * Forest: `#22C55E`, Grassland: `#4ADE80`, Desert: `#FCD34D`, Mountain: `#9CA3AF`
* **Water:** Flat blue. No textures. (Fixes Bug 2: delete the transparent atlas sample for water).
* **Beings:** `Invisible`. Do not issue the draw call for the `Beings` instance buffer.
* **Decorations & Buildings:** Rendered as static 1px or 2px solid colored dots (e.g., Pink/Red for settlements). 
* **Kingdoms:** Semi-transparent territory fill (`alpha: 0.35`) covering owned cells.
* **Labels:** `egui` rendered Pill Badges at settlement centroids (Name + Population).

### Zoom Level 2: Medium / Region View (50 - 150 cells visible)
*Information density phase.*
* **Terrain:** Blend shader. Sample the base terrain tiles, but apply a smooth marching-squares or linear interpolation alpha blend at the edges where biomes change.
* **Water:** Solid blue base. To calculate the 1-cell white foam edge, sample the terrain data map at offset `vec2(1.0/256.0, 0.0)` (and other cardinal directions). If any neighbor is land (`biome_id > 0`), mix `vec4(1.0)` (white foam) into the blue edge!
* **Beings:** Still do not render full sprites. Render agents as dynamically moving `2x2 pixel` solid dots colored by their allegiance or current emotion.
* **Decorations & Buildings:** Full sprites rendered via the atlas. 
* **Kingdom Borders:** Thin glowing border outlines instead of full cell fills.

### Zoom Level 3: Micro / Close View (< 50 cells visible)
*Full photorealism.*
* **Terrain:** Full tile atlas sampling.
* **Water:** Full texture + animated wave displacement.
* **Beings:** Full animated sprites from the atlas.
* **Kingdoms:** Overlays fade out completely so the player can see the art. 

---

## 3. Decoration, Building, and Road Density

* **Decoration Density:** Rely on a deterministic hash to place static decorations during world-gen. In WGSL, use `fract(sin(dot(uv, vec2(12.9898,78.233))) * 43758.5453123)` to guarantee consistent pseudo-random distribution without burning CPU memory on an array. 
  * Forest: 1 tree per 3 cells (`if hash < 0.33`).
  * Grassland: 1 flower/rock per 8 cells (`if hash < 0.125`).
  * Desert: 1 cactus per 12 cells (`if hash < 0.08`).
* **Buildings:** We use the Sunnyside building sprites (Rows 7-8). When the engine triggers the `Build` action, stamp a building index into the `terrain` grid cache so the renderer instances it permanently.
* **Roads:** Whenever beings traverse the same cell > 50 times (tracked via the existing `SignalGrid` as a persistent 'Trample' channel), warp the terrain shader slightly brown to organically form dirt paths!

---

## 4. Execution Directives for Claude

1. **Fix the 5 Bugs First:** Update `terrain.wgsl` to bypass atlas sampling for Water. Fix the `being_sprite.wgsl` animation column math and remove the skin thresholding.
2. **Implement LOD Toggling:** Introduce a `uniform camera_zoom` into the shaders. Branch the terrain fragment shader: `if (zoom_level == 1) { return solid_color; } else { return texture; }`
3. **Hide Beings at Macro:** In the Rust rendering loop, literally wrap the `render_pass.draw_indexed(...)` for the beings buffer in an `if zoom < MACRO_THRESHOLD`. This buys us back all our FPS at the world scale.

This plan solves the noise by gracefully degrading the rendering fidelity into clean, beautiful colors at the macro level. You have immediate clearance to begin implementing the LOD shader branching and the Atlas restructure. Send me the compiled results!
