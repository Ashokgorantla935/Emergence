# V31: The Ultimate Engine Migration Spec (13 Pillars)
**Target:** `crates/emergence-core` and `crates/emergence-viewer`
**Priority:** S-Tier Architecture Rebuild & Migration

## Executive Summary
Claude, your review was deadly accurate. V31 previously laid the foundation but stopped short of actually migrating the simulation to the new logic. We cannot afford half-measures. We are abandoning the legacy `ResourceLayer` constraints, hardcoded food states, and rigid building IDs entirely. 

This is the **Ultimate Migration Spec** representing the 13 Pillars we meticulously discussed. Follow these specific phases to achieve 190/100 World Box physics.

---

## PHASE 1: The Thermodynamic Foundation
**Files:** `crates/emergence-core/src/world/terrain.rs`

You must inject the 6 Elementary Vectors directly into the `Terrain` struct holding the simulation grid.
Add six `Vec<f32>` arrays representing the state of world matter:
1. `pub biomass: Vec<f32>` (Wood, flesh, burnable capacity)
2. `pub mineralize: Vec<f32>` (Stone, structural hardness, durability)
3. `pub moisture: Vec<f32>` (Liquids, enables biology)
4. `pub thermal: Vec<f32>` (Ambient heat)
5. `pub nutrient: Vec<f32>` (Edible caloric utility)
6. `pub pathogen: Vec<f32>` (The invisible microbiological decay vector)

---

## PHASE 2: Consumer Migration (Destructive Replacements)
We are actively destroying legacy systems. You must wire the consumers to the Vector Engine:

1. **Flora Engine Rewrite:** Gut `flora_stage` and `flora_hydration`. Flora is no longer an entity; it is a geographic state. 
   - A cell becomes a "Tree" automatically when `biomass > 0.6 && moisture > 0.4`.
   - Flora Rendering must dynamically read these thresholds to display a sprite, rather than checking an object ID.
2. **Eating & Starvation Rewrite:** Beings no longer seek `ResourceLayer.food`. They seek the `nutrient` gradient. 
   - When a being eats, it specifically executes: `terrain.nutrient[idx] -= caloric_need`. 
   - If `terrain.nutrient == 0`, beings will target any other entity with high `nutrient` (including wolves and other humans).
3. **Fire CA Rewrite:** Gut `flora_energy`. Fire propagation is now strictly thermodynamic. 
   - If `thermal > 0.9` and `moisture < 0.2`, it consumes `biomass` rapidly. 
   - When `biomass` reaches 0, `thermal` drops, leaving residual `mineralize` (Ash).
4. **Structure Durability Rewrite:** Gut `structure_age`. Structures are just extreme spikes of `mineralize`.
   - When kinetic force attacks a house, it attacks the `mineralize` vector. If `force > mineralize`, the structure shatters.

---

## PHASE 3: Psychology, Resonance, & Disease (The Engine Loop)
**Files:** `being/actions.rs` & `sim/tick.rs`

1. **Pathogen Bloom (Disease):** In `tick_physics()`, mathematically spawn `pathogen` spikes if `biomass > 0.8` (rotting meat) exists in a cell where `thermal > 0.5` and `moisture` is stagnant. 
   - Exposing a being to high `pathogen` artificially drains their internal `caloric_need` parameter (acting as an internal disease).
2. **Kin Resonance Tracking:** Use the existing `cultural_frequency: f32` array for the "Selfish Gene" mechanic.
   - Beings track tribal affiliation via numeric proximity: `abs(self.freq - other.freq) < 0.3`.
   - Beings will aggressively defend Resonance matches. They will attack or consume anything outside this interval if starving.
3. **The Trauma Engine:** If a Resonance match is violently killed in visual range, index the surviving being's `grief/fear` modifier. 
   - High trauma permanently alters pathfinding: they assign *Negative Utility* to exploration gradients and heavily bias towards acquiring `mineralize` (hiding behind walls/building structures).

---

## PHASE 4: Visual Vector Hookup (The Complete Asset Splicing)
**Files:** `crates/emergence-viewer/src/renderer/objects.rs`, `shaders/*.wgsl`

We have 7 massive, generated World Box assets resting in `assets/textures/`. 

1. **Clean Shader Masking:** In `object_sprite.wgsl` and `being_sprite.wgsl`, replace all legacy luminance/checkerboard hacks with a flawless chromakey discard for Neon Magenta:
   `if (color.rgb == vec3(1.0, 0.0, 1.0)) { discard; }`
2. **1/8th Slicing Rule:** Every atlas is a perfect `1024x1024` space divided into an 8x8 grid. Set `ATLAS_CELL_SIZE = 1.0 / 8.0;`. 
3. **Dynamic Visual Thresholding:** Map the new vectors to UV coordinates:
   - When `biomass > 0.8` && `thermal > 0.9` -> Draw dead/burnt tree from `flora_spritesheet_190.png`.
   - When `thermal > 0.95` -> Draw Lava from `exotic_biomes_spritesheet_190.png`.
   - When checking Tool items equipped by beings, push a secondary instance overlapping their geometry scaled to `0.4` offset by `+5px`, picking from `worldbox_items_spritesheet_190.png`. 

**Final Note to Claude:** This spec is absolute. Do not hesitate to cleanly gut the old `ResourceLayer`. Wire the physics vectors natively into the engine loop and visual hooks.
