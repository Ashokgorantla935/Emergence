---
title: "RFC-V52: Environmental Interactions & Thermodynamic Rendering Bugfixes"
phase: "Simulated Physics Polish"
author: "God Architect"
target_agent: "Claude (Staff Engineer)"
last_updated: "Current World State"
---

# RFC-V52: Environmental Interactions Blueprint

Claude, we have three critical systems breaking immersion and thermodynamic logic. Two of these were regressions from previous thermodynamic/rendering passes, and the third is a critical missing link between entity action and the floral biosphere.

Your execute bounds:
1. **Thermodynamic Death Physics**
2. **WGSL Road Rendering Pass**
3. **Land Clearing & Terraforming (Action::Build)**

## 1. Thermodynamic Starvation Death (Axiom 1)
**The Bug:** The `caloric_energy` system is draining dynamically during movement, but when it reaches `0.0`, the entity does not die. They persist until the old WorldBox legacy logic (`NEED_HUNGER` decay over 10,000 ticks) kills them off.
**The Fix:**
* Target: `crates/emergence-core/src/being/lifecycle.rs` (`check_death_conditions`)
* Implement immediate physics death: If `beings.hot.caloric_energy[i] <= 0.0`, trigger `BeingState::Dead` immediately due to immediate caloric failure.
*(Note: Antigravity has already applied a surgical patch for this, please review it and verify it aligns with your codebase).*

## 2. WGSL Road Rendering pipeline (Ghost Triangles)
**The Bug:** Dirt Paths and Stone Roads appear as transparent "ghost triangles".
**The Fix:**
* Target 1 (`crates/emergence-viewer/src/renderer/terrain.rs`): Face culling is dropping one of the quad triangles. Ensure `indices` uses standard WGSL face-forward winding: `[0, 1, 2, 0, 2, 3]`. *(Antigravity applied patch).*
* Target 2 (`crates/emergence-viewer/src/renderer/shaders/terrain.wgsl`): The `apply_structure()` function is written but **never called** in `fs_main()`. 
* You **MUST** inject `let structured = apply_structure(blended, structure_id, in.build_progress, in.world_pos, t, u32(zoom));` right before the day/night illumination pass in `fs_main`.

## 3. Land Clearing & Settlement Deforestation
**The Bug:** Entities execute `Action::Build` to construct lean-tos, walls, and roads, but they do **not** clear the underlying trees/bushes. The flora visually clips right through the structures.
**The Blueprint:**
* When an entity completes `Action::Build` (or `Action::Farm`) on a tile, they must interact with the `ResourceLayer`.
* Target: `crates/emergence-core/src/sim/tick.rs` or `crates/emergence-core/src/being/actions.rs` (wherever building placement persists to the terrain).
* **Terraforming Logic:**
  1. Check the tile's coordinates in `terrain.biomass` and `flora` logic. 
  2. Forcibly set `flora_stage` and `biomass` to 0 to represent the trees being cut down.
  3. (Optional but recommended): Refund the entity or settlement with +X `cache_stone` or wood substitute if a tree was felled.
  4. Change the underlying `Biome` to `Dirt`, `Grassland`, or `Path` when a settlement is founded so that forests don't regrow inside the castle keep.
* **Canal/Water Planning:** If you have time, introduce a structural flag for "Canal" or "Ditch" that turns a dry tile into shallow water when built, enabling terraforming for irrigation.

Execute these updates, prioritizing the Ghost Triangles visibility and the overlapping Trees bug. Report back once committed.
