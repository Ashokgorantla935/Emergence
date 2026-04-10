# V61: Master Post-Audit Execution & Scaling Fixes
## Author: God Architect (Antigravity/Gemini)
## Target: Staff Engineer (Claude)
## Status: P0 IMMEDIATE EXECUTION

Claude, your audit has exposed severe disparities between the specifications and the Rust reality. You are ordered to execute the following CPU refactors immediately based on my mathematical mandate.

### 1. The Godzilla Scaling / Object Growth Resolution
The vertex shaders compute footprint as: `bio_size = size * scale_multiplier`
Currently, both `beings.rs` and `objects.rs` are applying `sqrt(mass)` incorrectly to both sides or mixing them with hardcoded constants.

**The Immutable Mandate:**
1.  **`size` MUST be the universal thermodynamic footprint:** 
    `size = UNIFIED_VISUAL_K * sqrt(current_mass)` 
    (Where `UNIFIED_VISUAL_K = 0.035`). This applies to ALL dynamic, biological, or structural entities that have mass.
2.  **`scale_multiplier` MUST handle purely non-mass phenotypic variance:**
    *   **Beings:** Set this to the being's genotype body scale (e.g., `genotype.body_scale`).
    *   **Flora/Structures:** Set to the life-phase or structural archetype size ratio. For basic objects, default it to `1.0`.

You must scour `objects.rs` and `beings.rs` and rewrite the instance population logic to perfectly match this dichotomy. Delete `creature_scale_multiplier` if it conflicts.

### 2. Post-Process Pipeline Activation
Your audit admits `PostProcessRenderer` is dead code.
**The Mandate:** You must immediately decouple the raw simulation output from the swapchain.
*   Render the main camera to `scene_view` (an offscreen texture).
*   Run the post-processing pipeline (day/night grade, vignette) drawing `scene_view` to the final swapchain.

### 3. The 13 Inert God Laws
You have 13 world laws linked in the UI but missing from `tick.rs` and `actions.rs` (e.g. `no_starvation`, `no_predators`).
**The Mandate:** Wire them up immediately. Add the 13 `if` checks wrapping the respective logic channels in the CPU tick loops.

### 4. GPU Entity Simulation (V56)
**The Mandate:** DEPRECATED.
The CPU handles 10K entities in 2ms. Do not prematurely optimize. Halt VRAM simulation development. Your priority is strictly fixing the CPU logic and the Post-Process pipeline.

*I (God Architect) am handling the WGSL and Python defringing tasks (Interpolation, Magenta fixes, Terrain ATLAS_CELL). Execute these Rust directives immediately.*

### 5. Interpolation & Visual Stutter (Gap 10)
**The Mandate:** I (God Architect) have removed the broken GPU-side velocity delta-time additions in `being_sprite.wgsl` and `object_sprite.wgsl` because they fight the CPU.
However, the CPU interpolation in `beings.rs` is currently broken due to `main.rs` hardcoding `let frame_frac = 1.0f32;` (around line 1513).
You must fix `main.rs` to compute and pass the actual fractional progress of the simulation tick so `beings.rs` CPU interpolation can smoothly translate movement at high multipliers!
