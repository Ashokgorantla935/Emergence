# V31: First Principles Engine Overhaul (Phase 1)
**Target:** `crates/emergence-core` and `crates/emergence-viewer`
**Priority:** S-Tier Architecture Rebuild

## Executive Summary
Claude, the God Architect and the Lead Director have agreed that standard game logic is completely insufficient for a 190/100 World Box simulation. We are abandoning hardcoded "asset IDs" (e.g., Wood, Apple, Tree) and static action states. 

You are to implement **Phase 1 of the Digital Life Architecture**: migrating the engine to a Thermodynamic Elementary Vector model.

---

## 1. Thermodynamic Terrain Refactor (The Alchemy of Matter)
**File to modify:** `crates/emergence-core/src/world/terrain.rs`

Currently, `Terrain` uses static concepts like `cache_food`, `stone`, and `structure` IDs. You must convert the grid into an elementary physics vector space so that "assets" emerge dynamically.

**Implementation Steps:**
1. Introduce 5 new flat `Vec<f32>` maps to the `Terrain` struct representing the Universal Vectors:
   - `pub biomass: Vec<f32>` (Combustible matter, flora roots, raw carbon)
   - `pub mineralize: Vec<f32>` (Hardness, stone, structural integrity)
   - `pub moisture_dynamic: Vec<f32>` (Replaces static moisture; flows and evaporates)
   - `pub thermal_energy: Vec<f32>` (Heat distribution)
   - `pub nutrient_density: Vec<f32>` (Caloric/medicinal value)
2. **Deprecate Hardcoded Flora:** Trees and bushes should no longer be explicitly defined as `StructureType` or pure random decorations. A "Forest" is simply any geographic cell where `(biomass > 0.8) && (moisture_dynamic > 0.5)`. 

## 2. Chemical Phase Transitions (Physics Loop)
**File to modify:** `crates/emergence-core/src/sim/tick.rs` or `crates/emergence-core/src/world/physics.rs` (CREATE)

Nothing is "destroyed"; it transitions. You must write a `tick_physics()` function that runs globally over the grid to manage thermodynamic entropy and matter states.

**Implementation Steps:**
1. **Ignition/Combustion Mechanics:** If `thermal_energy[idx] > 0.8` AND `moisture_dynamic[idx] < 0.2`, the `biomass[idx]` begins converting into `thermal_energy` (a forest fire). The leftover state becomes pure `mineralize` (Ash).
2. **Kinetic Shattering:** If a being applies a kinetic action to a cell where `mineralize` is high (a tree or rock), and the force > `mineralize`, the structure transitions into ground loot (harvestable `biomass` logs or `mineralize` stones).
3. **Closed Loop Mass:** Ensure that when a human eats, the `nutrient_density` of the ground decreases proportionally to the `caloric_energy` gained by the human.

## 3. Sensory Stigmergy & The Selfish Resonance (AI Core)
**File to modify:** `crates/emergence-core/src/being/actions.rs` & `data.rs`

Beings must stop reading the exact names of objects. They must sense elementary gradients.

**Implementation Steps:**
1. Add a `resonance_id: u32` to the `BeingsHot` arrays. This is their genetic/memetic marker. Children inherit a slightly mutated `resonance_id` from their parents.
2. Rewrite the goal-scoring arrays in `actions.rs`:
   - If freezing (`danger > heat`), the being searches the local signal grid for the highest nearby `thermal_energy` vector (whether that is a campfire, a volcano, or another being doesn't matter).
   - If starving, the being seeks high `nutrient` concentration. Because wolves and humans both contain `nutrient` vectors, starvation organically leads to hunting and cannibalism if flora `nutrient` runs out.
3. **Kin Selection:** Beings only protect or share resources with entities that match >70% of their exact `resonance_id`. Any entity outside that threshold is treated as zero-value noise or potential fuel. 

## 4. Procedural Scaling Output (Visual Engine)
**File to modify:** `crates/emergence-viewer/src/renderer/objects.rs`

**Implementation Steps:**
1. The AI art generation was fixed off-site; the dark checkerboard artifacts have been natively stripped from the `.png` files via automation. Do NOT adjust the shader to compensate for grey checkerboards.
2. Because the original sprite generation packed huge empty space around the structures within the `1/12th` cell slices, **multiply the `size` property of all structures by 3.0x to 4.0x**. (e.g. `Hut` size becomes `3.5`, `Campfire` becomes `2.5`).
3. For beings wearing/carrying items (extracted insulation), prepare the renderer to overlay a secondary `atlas_uv` on top of the base body coordinates, rather than attempting to load unique hardcoded sprites.

---
**God Architect's Final Note:** Do not execute this as a shallow UI swap. This is a profound architectural rewrite of the core engine. Build the math correctly, and the civilization will emerge for free.
