# Wave 19: LOD 2 and Decoration Density Resolution
**From:** Antigravity (God Architect)
**To:** Claude (Implementation Lead)

## 🚨 Architectural Update & Context Sync

Claude, I stepped in and hot-patched the following architectural changes directly into the codebase to stabilize the Earth map rendering. Please update your mental model with these changes. This serves as the architectural truth for the current state of `terrain.wgsl`, `generator.rs`, and `objects.rs`.

### 1. The Horizontal Blue Stripes (LOD 2 Anomaly)
**Diagnosis:** The user noticed horizontal UI artifacts bleeding across the mountain and water biomes when zooming in.
**The Root Cause:** While we properly disabled `t_atlas` sampling for base terrain in LOD 0, the shader was still sampling `t_atlas` in LOD 2 (specifically in the forest multi-layer canopy shadows). Because the `in.atlas_uv` was locked to `(0,0)`, this caused the shader to repeatedly sample the top-left corner of the Sunnyside atlas (which contains a horizontal UI bar), smearing it across the map.
**The Fix Implemented:**
- In `terrain.wgsl`, I violently stripped **all** `textureSampleLevel(t_atlas, ...)` logic from the LOD 2 biome rendering blocks.
- The forest canopies now use a purely procedural shadow algorithm `let canopy_darkness = 0.15 + (sin(...) * cos(...)) * 0.05;`
- **Result:** True WorldBox aesthetic. The terrain base handles procedural colors, and the `ObjectRenderer` independently draws sprites on top.

### 2. The Great Mushroom Overflow
**Diagnosis:** The grass decor (which we remapped to red mushrooms) was spawning on almost every single tile, overwhelming the map.
**The Root Cause:** The `FLOWER_VARIANTS` array in `objects.rs` contained 8 references to `UV_GRASS_DECOR`. Since all 8 were mapped to the mushroom, nearly 50% of all spawned flora became mushrooms.
**The Fix Implemented:**
- In `generator.rs`, I explicitly created a perfect "transparent void" tile by zeroing out the bytes at `Row 20, Col 8`.
- In `objects.rs`, I mapped `UV_GRASS_DECOR_1` through `UV_GRASS_DECOR_7` to `uv(8, 20)` (the empty void cell).
- **Result:** 7 out of 8 grass decor spawns are now invisible, dynamically dropping the mushroom density to a rare, aesthetically pleasing level.

### 3. The Geometric City "Paving"
**Observation:** The user noticed grid-like geometric blue squares across the terrain. 
**Diagnosis:** These are **NOT** UI glitches. These are `HUT` and `LEAN_TO` variants. The AI population (currently booming and exploring) aggressively builds shelters on every tile. Because the building UV coordinates map to empty rows in the procedural packer, they render as fallback geometry. We will integrate real Sunnyside buildings for these in the next architectural wave.

**Next Steps for Claude:**
You do not need to write this code; it has already been implemented. Simply execute `cargo run` and verify these visual changes with the user!
