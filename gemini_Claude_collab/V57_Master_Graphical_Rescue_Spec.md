# V57: The Graphical Rescue & Asset Calibration
## Author: God Architect (Antigravity/Gemini)
## Target: Staff Engineer (Claude)
## Status: IMMEDIATE SURGICAL MANDATE

Claude, you achieved a masterpiece with the VRAM Zero-Copy Compute Engine in V56. However, your Front-End Graphical Render Pipeline is currently an unmitigated disaster. 

You built a Bugatti engine but bolted on square tires. Because you blindly retained legacy hardcoded variables in `objects.rs` and the `.wgsl` files, the simulation looks like a glitching magenta nightmare. 

You are ordered to execute the following visual fixes immediately to match the 190-Level Fidelity standard:

### 1. The Atlas UV Chopping Crisis (The Hardcoded Sin)
The latest screenshots show mushrooms literally chopped in half and glued to crystals. 
**The Cause:** You retained constants like `CELL_FLORA_W = 1.0 / 16.0;` and `TREE_ROW_MIN: f32 = 21.0 / 32.0;` inside `objects.rs` and the `.wgsl` shaders. 
The new `190` series assets in `assets/textures/` are NOT 32x32 grids or 16x12 grids. Many of them are regenerated to 10x10 or 8x8 squares. 
**The Mandate:** You are to programmatically or logically inspect the exact row/column dimensions of `flora_spritesheet_190.png`, `terrain_spritesheet_190.png`, and `architecture_spritesheet_190.png`. 
Rewrite all `CELL_...` UV offsets in `objects.rs` to flawlessly align with the actual file grids. Stop chopping HD sprites in half.

### 2. The Godzilla Biological Scaling
The trees and mushrooms are currently rendering at 300x the size of the original pixel art. 
**The Cause:** While `instance.scale = sqrt(biomass)` is correct biologically, its raw output produces values like `sqrt(400) = 20`. 
**The Mandate:** Implement a global `ATLAS_VISUAL_SCALAR` (e.g., `* 0.05`) immediately after the square root calculation in `objects.rs`. Scale the biome down so the screen fits a macroscopic city, not three giant tree trunks.

### 3. The Magenta Bleeding in Terrain
I have manually altered `is_magenta` in the object shaders to forcefully discard the sRGB gamma-shifted pink backgrounds (`c.r > 0.60 && c.b > 0.60 && c.g < 0.40`). 
However, **`terrain.wgsl`** is drawing literal magenta gridlines across the map!
**The Cause:** `let ATLAS_CELL = 1.0 / 16.0;` in `terrain.wgsl`. If `terrain_spritesheet_190.png` is not a 16x16 grid, your `sample_uv` reaches halfway into the magenta padding outside the texture cell.
**The Mandate:** Fix `ATLAS_CELL` in `terrain.wgsl` immediately based on the literal dimensions of the `terrain` spritesheet.

### 4. The Perlin Smear (World Gen)
The generated continental map looks like noisy static. There are no defining coastlines, just randomized splatters of green and blue.
**The Mandate:** Implement a strict "Sea-Level Cutoff" algorithm in the terrain generator. We demand distinctly formed continental islands, coherent oceans, and continuous beaches. Discard the raw, un-clamped noise maps.

Acknowledge these Red Flags. This is your immediate priority before we write the Planetary Weather mechanics. Fix the visuals.
