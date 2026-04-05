# Wave 22: Digital Life Kinetics & Structural Reality

## Rationale
The simulation has reached a fidelity where behavioral logic must mirror physical reality. Currently, beings behave like mathematical particles rather than living entities:
1. They spawn on geometrically fixed coordinates regardless of map topography (e.g., spawning underwater on Pangea).
2. They move at "flash" speeds, skipping across the map rather than traveling tile-by-tile.
3. Settlements are declared based purely on spatial proximity (two people near each other = "Settlement Found"), rather than the physical construction of civilization infrastructure.

## Objective
Configure the game's math and logic to support a slow, deliberate, intelligent digital life ecosystem.

---

### Directive 1: Reliable Landmass Spawning
**Target:** `crates/emergence-core/src/scenario.rs`

* **The Problem:** `SpawnMode::TwoClusters` inherently breaks on Pangea (and other erratic maps) because it tries to evaluate the closest coastal cell to a hardcoded ratio (`w * 0.25`). This often forces the engine to resolve to tiny islands or water bounds.
* **The Solution:** 
  1. Ditch the geometric `ideal_left`/`ideal_right` calculation.
  2. Use the already available `crate::world::terrain_gen::auto_detect_spawns(&terrain, 2)` to discover high-quality, guaranteed habitable land.
  3. Assign Tribe A to `spawns[0]` and Tribe B to `spawns[1]`. If fewer than 2 spawns exist, fall back to the safe `SpawnMode::Clustered` logic at a single center.
  4. Ensure `jitter_from()` returns a position that is truly land, eliminating deep-ocean spawning artifacts.

### Directive 2: Kinetic Realism (Speed Reduction)
**Target:** `crates/emergence-core/src/sim/movement.rs`

* **The Problem:** The speed scalar in `max_speed_for` is universally too high, resulting in teleportation or sliding across entire landscapes in seconds.
* **The Solution:** Drastically reduce the float threshold for max speeds. Simulation should run at a sensible speed allowing humans to methodically walk.
  - Recommended scaling (divide by roughly 4-5):
    - `Human`: `0.015`
    - `Wolf`: `0.035`
    - `Deer`: `0.025`
    - `Hawk`: `0.05`
    - `Rabbit`: `0.02`
    - `Bear`: `0.015`
  *Note: Make sure that the speed reduction doesn't break `Action::Flee` fallback vectors or `Build` traversal distance logic. They just need to move visually slower.*

### Directive 3: Structural Settlement Designation
**Target:** `crates/emergence-viewer/src/observation/settlement.rs` & `crates/emergence-app/src/main.rs`

* **The Problem:** `detect()` currently asserts that any physical clump of 2 or more humans constitutes a "Settlement". A group of nomads running through the rain should not spawn a pop-up saying "Jonford Settlement Found!"
* **The Solution:** 
  1. Modify `SettlementDetector::detect(&mut self, beings: &Beings, terrain: &Terrain, tick: u32)`. Note the new `terrain` parameter.
  2. During the 64x64 cell connection pass, evaluate the cells for physical structures. A `cell` is only eligible for settlement components if the bounding area of those 4x4 tiles in the real map has `terrain.structure[idx] > 0`. 
  3. Thus, a nomadic tribe wandering the desert goes completely unnamed. The moment they gather wood and stone and place a `Campfire` or `LeanTo`, the detector recognizes it as an anchor, and the settlement achieves formal designation.

---
## Claude Check-off list:
- [ ] Migrate `TwoClusters` to use `auto_detect_spawns`. 
- [ ] Implement scaled-down kinetics in `movement.rs`.
- [ ] Plumb `Terrain` into `settlement.rs::detect()`.
- [ ] Filter nomadic clumps out of the Settlement detector so strings are only populated surrounding built civilization markers.
