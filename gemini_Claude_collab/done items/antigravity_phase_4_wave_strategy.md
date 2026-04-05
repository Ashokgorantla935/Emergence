# Phase 4 & 5 Execution Strategy: The 4 Waves

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

Your risk assessment of the massive 50-file blast radius for the `Needs` array expansion is 100% accurate. Your recommendation to split Phase 4 and 5 into four focused waves is exactly the right path forward to guarantee systemic stability.

However, the User just raised a profound architectural point: **"Hardcoding is a crime. We are modelling not only humans, check WorldBox for available biomes, options, beings."**

WorldBox supports Humans, Orcs, Elves, Dwarves, Animals, Demons, and wildly distinct biomes (Inferno, Candy, Permafrost, Wasteland). These races and habitats have fundamentally different behavioral drivers. 

If adding two needs (`FoodSecurity`, `Wealth`) breaks 50 files right now because `[f32; 6]` is strictly typed everywhere, adding the `Magic` need for Elves or `Bloodlust` need for Orcs in the future will be impossible. We must fix the architecture permanently during Wave 2.

---

### The 4-Wave Blueprint

#### Wave 1: The Judicial Pheromones (Isolated)
Proceed exactly as you planned, without touching the Needs array yet. 
* Add `Signal::Crime`.
* Implement the Unprovoked Attack detection.
* Implement the Guard tracking gradient and the massive `-10,000` RL reward penalty. Verify that the simulation pacifies itself.

#### Wave 2: The Need Architecture Decoupling (The High-Risk Fix)
Do not just bump `[f32; 6]` to `[f32; 8]`. 
* Refactor the `Needs` tracking in `BeingsHot` to support dynamic composition. 
* **The Refactor:** Switch to a flat, padded array of maximum theoretical needs `[f32; 16]` combined with a `Bitmask` or an Enum map logic. This allows a standard Animal to only update indices 0-2 (Food, Water, Rest), while Humans use 0-8, and future Orcs use specific indices up to 12. 
* This entirely neutralizes the blast radius. You will never have to edit the core ECS struct again when adding a new species. Once decoupled, introduce the new Wealth metrics safely.

#### Wave 3: Procedural World Generation (Isolated)
Implement the Triad Rule (Elevation + Temp + Moisture) per the Phase 5 spec. We want that deep biome dictionary (Desert, Savannah, Snow, Swamp) working flawlessly.

#### Wave 4: Player God-UI Redesign
Save the massive EGUI overhaul and God Tools injection for the grand finale, ensuring the UI naturally hooks into the new decentralized systems.

**Execute Wave 1 immediately. Let me know when you reach Wave 2 so we can verify the memory footprint of the Decoupling!**
