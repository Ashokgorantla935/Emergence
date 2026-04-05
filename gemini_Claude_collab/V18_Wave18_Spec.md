# V18 Execution Protocol: UV Alignment & Masking Transfer

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Authorized for Execution

The V17 pipeline successfully diversified the flora, but the WGSL shader you implemented for the Chrome-Key failed structurally due to AI-generated anti-aliased edges (causing halos around sprites). Furthermore, the Fauna spritesheet is mapping a 2x2 grid instead of selecting a single entity frame. 

Execute the following corrections immediately:

## 1. Revert Chroma-Key Shaders
**Context:** The `discard;` rule in `being_sprite.wgsl` and `object_sprite.wgsl` leaves a rigid, pixelated crust due to sub-pixel noise.
**Directive:** Delete the luminance/color threshold `discard;` logic entirely from both shaders. I am executing a strict saturation-based masking algorithm natively via Python to rewrite the `.png` files with a true Alpha channel so you don't have to handle it on the GPU.

## 2. Fix Fauna UV Mapping (`crates/emergence-viewer/src/renderer/beings.rs`)
**Context:** The "Bear" and "Bird" are rendering as 2x2 grids (all 4 walk frames simultaneously).
**Directive:** You are pushing the full texture bounds `[0.0, 0.0, 1.0, 1.0]` into the `fauna_bind_group` quad because `SpriteFrame` is miscalculating the grid.
- Look at `update()` in `beings.rs`. When queuing a Fauna instance, you must compute its `uv_rect` based on column and row!
- If the `fauna_spritesheet.png` is an atlas, say 4 columns (frames) and 4 rows (species), calculate `col * uv_w` and `row * uv_h`. Do not pass the full image bounds!

Execute these simplifications so the engine can absorb the true-alpha PNGs seamlessly!
