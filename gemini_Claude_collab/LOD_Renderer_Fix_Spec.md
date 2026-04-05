# V5 Renderer Protocol: Unmasking the Artifacts (Revision 3)

**To:** Claude
**From:** Gemini (Architect Level)
**Status:** High Priority UI/Renderer Fix

Claude, excellent cross-verification on the `terrain.wgsl` base shader. You correctly identified that `atlas_uv` is already grounded to zero, meaning the physical grid artifacts the user continues to see are 100% stemming from the `ChunkedObjectRenderer` deploying perfectly mathematically-dense resource arrays.

The reason the spreadsheet look is inescapable even when completely zoomed in—is that `Resource` instances (like Grain and Stone) have a native `size = 1.8` relative to a cell size of `1.0`. They literally physically overlap every adjacent cell and form a mathematically impenetrable visual wall because the density of resource generation covers practically every grid slot in its biome. Add to that the `max(size * ppu, 6.0) / ppu` logic, and at macro zoom, they multiply to 10.0+ width sizes creating lag/overlapping solid visual masses.

Here is the finalized execution scope.

---

## Issue 1: Zoom Clamping Inflation (The Lag & Macro Overlap)
**The Cause:** In `being_sprite.wgsl` and `object_sprite.wgsl`, sprites are constrained to a minimum of 6px/8px via `max(inst.size * camera.pixels_per_unit, 6.0) / ppu;`. When `ppu` drops below 1.0 at macro scales, sprites inflate their world-space bounding boxes colossally, covering up to 20x20 cell areas each causing huge overdraw and overlapping matrix effects.
**The Fix:** 
Modify the fragment shaders (`fs_main`) in both `object_sprite.wgsl` and `being_sprite.wgsl` to institute an explicit LOD culling threshold *before* the pixel clamping wreaks havoc.
```wgsl
    // In vs_main: compute raw scale
    let raw_pixels = inst.size * camera.pixels_per_unit;
    // (Pass this to fs_main)
    
    // In fs_main: Cull out items that are naturally sub-pixel
    if (in.raw_pixels < 2.0) {
        discard;
    }
```

## Issue 2: Synchronous Frame Drop (`ppu_changed`)
**The Cause:** `ChunkedObjectRenderer::update` actively tracks `(self.pixels_per_unit - pixels_per_unit).abs() > 1.0` and triggers `dirty = true` on ALL chunks simultaneously during scroll wheel interaction, lagging the engine.
**The Fix:** 
Strip `ppu_changed` evaluation entirely from `objects.rs`. Object arrays do not mutate physical placements based on `ppu`. Let the fixed `120` frame interval handle natural background chunk regeneration, and rely entirely on the new wgsl pixel `discard` logic to vanish micro-entities dynamically. 

## Issue 3: The Micro-Zoom "Spreadsheet" Aesthetic
**The Cause:** When closely zoomed in, the `FoodType::Grain` and `Stone` resources deploy on nearly every cell with a size roughly double the cell coordinates (`1.8`). No amount of horizontal jitter breaks this apart—they form visually unbroken carpets of sprites mimicking spreadsheet tiles.
**The Fix:** 
Within the `resources` generation loop inside `ChunkedObjectRenderer::rebuild_chunk_standalone`, forcefully fragment the deployment density using the hash algorithm. Break up the mathematically perfect walls of continuous wheat and rock.
```rust
// Around line 545, within the resource loop inside objects.rs:
let hash = cell_hash(x, y);

// Only allow resources to spawn on ~40% of standard valid resource cells
if hash % 10 > 3 {
    continue; 
}

// Ensure organic jitter remains:
let jitter_x = ((hash % 17) as f32 / 17.0 - 0.5) * 0.6;
let jitter_y = (((hash >> 4) % 17) as f32 / 17.0 - 0.5) * 0.6;
```

Claude, please execute these directly into `object_sprite.wgsl`, `being_sprite.wgsl`, and `objects.rs`.
