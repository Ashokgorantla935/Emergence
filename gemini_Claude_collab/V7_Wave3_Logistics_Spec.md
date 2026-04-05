# V7 Protocol: Logistics & Pheromones (Wave 3)

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Wave 3 Initiation

Claude, incredible execution on Waves 1 & 2. The Fauna vector boids and dynamic Simplex cloud fields are a massive leap in ecosystem depth. Now, we must transition the civilization from wandering survivalists into a cohesive, organized society.

We are launching **Wave 3: Logistics & Community**. This phase explicitly removes global tech assumptions and forces Beings to rely on spatial logistics.

---

## Phase A: Action/Inventory States (`ActionContext`)
**File Target:** `crates/emergence-core/src/simulation/beings.rs`
Beings need to carry physical items and display observable intent. This bridges the mathematical simulation natively to the renderer.

1. **The Schema Upgrade:** Add a lightweight enums representation to the `Being` struct:
   ```rust
   // Replace or expand the `state: u8` with explicit physical interactions
   pub enum ActionContext {
       Idle,
       Wandering,
       CarryingWood,
       CarryingStone,
       CarryingFood,
       Building(u32, u32), // target grid x,y
       Sleeping,
       Fleeing, // Triggered by Danger/Cataclysms
   }
   ```
2. **The Logic:** Inside the pathfinding/task loop, if a Being decides to construct a `Hut`, they must first pathfind to a `ResourceLayer.flora_age > 1` (Tree) tile, chop it, transition to `ActionContext::CarryingWood`, and physically transport it to the construction coordinate.

## Phase B: The Pheromone Sharing Grid
**File Target:** `crates/emergence-core/src/world/signal.rs` & `memetic.rs`
We must avoid 10,000 independent campfires. Sharing naturally emerges through physical signal attraction.

1. **Structural Emissions:** Modify the Signal loop. If a coordinate physically contains a `StructureType::Campfire` or `Tavern`, it unconditionally mathematically emits an extremely strong `Comfort` and `Warmth` signal gradient out to a radius of 6 cells.
2. **Attraction Vectors:** In the Beings' physics desire logic (where they evaluate Needs), if a Being's `comfort` level is critically low, they drop their current task and override their movement vector to strictly climb the highest localized `Comfort` gradient. They will organically migrate toward community fires instead of independently building them.

## Phase C: Topographical Modification (The Paving Engine)
**File Target:** `crates/emergence-core/src/world/terrain.rs` 
Ingenuity is scarring the earth. 

1. **The Trample Integer:** You already have `trample: Vec<u8>` in `Terrain`. Inside the physical movement handling function where Beings shift from `[x1, y1]` to `[x2, y2]`, increment the `trample` value of their origin grid cell by `+1`.
2. **Dirt Routes:** If `trample` hits `[MAX: 255]`, forcefully convert the underlying cell's `Structure` buffer to `StructureType::DirtPath`. This physically paints a cohesive brown line representing heavy civilization traffic.
3. **Cobblestone Economy:** Add a new macro Builder evaluation loop: If the Kingom has access to stockpiled Stone (deep crust `cache_stone > 0`), and there is an existing `DirtPath`, Builder Beings target the `DirtPath`, spend the Stone, and structurally upgrade the tile to `StructureType::StoneRoad`.

---

**Claude, begin by executing Phase A and Phase C inside the primary core framework. Introduce the `ActionContext` tracking onto the actual `Being` entity memory layout, and wire up the `trample` incrementation locally. Provide your implementation overview before pushing the data.**
