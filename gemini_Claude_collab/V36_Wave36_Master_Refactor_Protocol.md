# V36: The Grand Unification Protocol (WorldBox 190/100)
**Target:** S-Tier Architecture Rebuild

## Executive Summary
Claude, this is your Master Directive bridging the philosophical 15-Pillars of Digital Life to cold, hard Rust logic. Do not execute these systems in a vacuum. You are to combine the destructive migration logic detailed below with the specific mathematical shaders and asset taxonomies we have already prepared across the V30-V35 spec series.

---

## Part 1: The Master Spec Index (Continuity)
You possess the following finalized component specs. You must read them in conjunction with this document:
- **V30 Digital Life Architecture:** Documents the 15 philosophical pillars of thermodynamic physics, memetics, and disease.
- **V32 Asset Taxonomy:** The 190/100 scale design categorization of every biome, flora, and ore phase.
- **V34 Walkthrough / Assets:** We generated 7 massive `1024x1024` sprite sheets natively in `assets/textures/`. 
- **V35 Visual Asset Binding:** Contains the EXACT WGSL code to chromakey the `#FF00FF` magenta background and the `ATLAS_CELL_SIZE = 1.0 / 8.0` math to extract the 8x8 grids.

---

## Part 2: The Core Struct Migrations
You must ruthlessly gut the legacy systems. Do not make this additive. Modify `Terrain` in `crates/emergence-core/src/world/terrain.rs`:

```rust
// RIP legacy `flora_stage`, `flora_hydration`, `flora_energy`, `structure_age`.
pub struct Terrain {
    pub dimension: u32,
    // The 6 Fundamental Elements (190/100 Core)
    pub biomass: Vec<f32>,     // Combustible/carbon density
    pub mineralize: Vec<f32>,  // Hardness/durability
    pub moisture: Vec<f32>,    // Hydration spread
    pub thermal: Vec<f32>,     // Ambient heat
    pub nutrient: Vec<f32>,    // Edible utility
    pub pathogen: Vec<f32>,    // Microbiological decay field
    // ... Any other non-deprecated legacy vectors
}
```

## Part 3: Existential Physics Execution (The 15 Pillars)
Wire the remaining 15-Pillar systems dynamically into the `tick` and `action` loops:

### 1. Thermodynamic & Flora Overhaul (Pillars 1, 2, 4)
*   **Flora Rule:** A cell automatically spawns a Tree rendering when `biomass > 0.6` & `moisture > 0.4`.
*   **Fire Propagation:** Expand `tick_physics()`. If `thermal > 0.9` & `moisture < 0.2`, instantly tick down `biomass` by 0.1 and increment `thermal`. Once `biomass` hits 0.0, zero the thermal and leave `mineralize` (Ash).

### 2. Starvation & Darwinian Evolution (Pillars 3, 6, 9)
*   **Gut `ResourceLayer.food`:** Beings pathfind purely against `nutrient`. If ground nutrient is zero, the gradient naturally points them to consume wolves or other humans.
*   **Birth RNG:** In `lifecycle.rs`, when a new being spawns, mutations occur: `child_speed = parent_speed * rng.gen_range(0.95..1.05)`. Allow cold biomes to naturally cull low-insulation offspring.

### 3. Entropy & Stolen Physics (Pillar 10)
Encode the mandatory heat drain formula in the lifecycle tick:
```rust
let heat_loss = (beings.body_temp[i] - terrain.thermal[idx]).max(0.0) / beings.insulation[i];
beings.caloric_energy[i] -= heat_loss * ENTROPY_CONSTANT;
```
Allow beings to increase their `insulation` variable by holding tools crafted from high-insulation dead fauna.

### 4. Resonance, Trauma & Trade (Pillars 7, 12, 14)
*   **Resonance Check:** Use `cultural_frequency: f32`. Two beings match if `(self.freq - other.freq).abs() < 0.3`.
*   **Trauma Math:** If a strict match dies violently in local sight, add a generic `grief` struct boolean to the survivor. Traumatized beings flip exploration gradients from positive to *Negative Utility* and actively seek `mineralize` walls (Isolation behavior).
*   **Trade Logic:** When encountering divergent `cultural_frequency` beings, cross-check physiologic needs. If A needs `thermal` and B needs `nutrient`, automatically swap their equipped items.

### 5. Memetics & Disease (Pillars 8, 13, 15)
*   **Pathogen Blooms:** Inject `pathogen_field` logic into `tick_physics`. If `biomass > 0.8` (e.g. dead bodies/meat) exists on a cell where `moisture == 0.0` (stagnant), `pathogen` increments. Exposure to `pathogen > 0.5` causes beings to lose maximum caloric utility limits.
*   **Dark Ages:** Track memetic states. If ambient `thermal > 0.5` satisfies a tribe, their action for "Build Fire" never fires. If unused for `14,400 ticks` (50 years), set their ability to trigger that action to 0.

### 6. Procedural Compositing & Visual Asset Binding (Pillar 11)
You must wire the 7 massive generated spritesheets we discussed (V32-V34) natively.
*   **Target Files:** `crates/emergence-viewer/src/renderer/objects.rs`, `beings.rs`, `shaders/*.wgsl`
1. **Flawless Chromakey (Discard Hack):** The 7 new assets use a pure Magenta background. In `object_sprite.wgsl` and `being_sprite.wgsl`, you must replace the old grey-saturation discard logic with strict hex tracking:
```wgsl
let base_color = textureSample(t_texture, s_texture, uv);
if (base_color.r == 1.0 && base_color.g == 0.0 && base_color.b == 1.0) {
    discard; // Perfect #FF00FF Masking
}
```
2. **Mathematical Slicing (1/8th):** All 7 spritesheets are exactly 1024x1024, arranged in an 8x8 grid. In your UV scaling matrix, you MUST set:
`const ATLAS_CELL_SIZE: f32 = 1.0 / 8.0;`
3. **Item Attachment Overlay:** Do not generate hardcoded "human_wearing_jacket" sprites. In `beings.rs`, to render equipped items, push a secondary `ObjectInstance` command to the GPU immediately after the Being's core command. Pull the specific tool from `worldbox_items_spritesheet_190.png`, scale it to `0.4`, and apply an XYZ geometric offset of `(+5px, +5px, +0.1)` overlaid on the character's geometry.

## Directive Conclusion
This is exactly how to achieve 190/100 scale architecture. Do not skip the RNG birth mutation, the Thermodynamic trade, or the Trauma Engine. Execute it immediately.
