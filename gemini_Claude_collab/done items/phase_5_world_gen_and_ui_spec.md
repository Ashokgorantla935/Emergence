# Phase 5 Spec: God UI & Procedural World Gen

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

We are overhauling the player interaction loop. We need tools to inject consequence into the Simulation Grid trivially, and we need a world generator capable of spinning up endless variations of continents.

## 1. Domain-Warped World Generation
Throw away the hard-coded Simplex arrays. The generator must mathematically assemble robust and varied biomes every time the user hits "New World". 

* Implement three independent `fastrand` initialized Perlin octaves inside the generator map builder:
  * **Elevation Map**
  * **Temperature Map**
  * **Moisture / Rainfall Map**
* The grid's Biome array is determined by combinatorial testing across the layers:
  * Elevation `> 0.8` AND Temp `< 0.3` -> `Snow Peaks`
  * Elevation `> 0.8` AND Temp `> 0.6` -> `Desert Highlands`
  * Elevation `< 0.3` -> `Ocean`
  * Elev `0.3-0.5` + Moisture `> 0.6` -> `Marsh`

## 2. EGUI Player Dashboard Refactor
The current UI is clunky. Rip the side-bar out and replace it with a professional EGUI suite tailored around rapid interventions:
* **The Global Timeline (Bottom Bar):** A horizontal frame spanning the bottom 50px featuring Time Speed buttons (`1x, 5x, 100x`), total Population tracking, and total global Wealth / Happiness metrics.
* **The God Tool Dock (Collapsible Tree on Right):**
  * Top Level: `Creation`, `Destruction`, `Civilization`. 
  * Under `Creation`: `Spawn Biome`, `Bless (-10,000 Need Decay)`.
  * Under `Destruction`: `Strike (100.0 Heat, creates fire)`, `Plague (100.0 Sickness)`, `Curse`.
  * Every button is physically mapped to an injection payload on the `Signal Grid`. The player is acting *inside* the chemical reaction-diffusion matrix of the world.
* **The Brush:** Hovering over the viewport while holding a God Tool projects a transparent circle. Upon Mouse Click, execute the spatial injection algorithm.

**Implement these two UI/Gen systems to finalize the game loop framework!**
