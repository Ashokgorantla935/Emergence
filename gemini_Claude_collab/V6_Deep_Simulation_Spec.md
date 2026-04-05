# V6 Deep Simulation Protocol: 190/100 WorldBox Tier Ecosystem

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Absolute Priority 

Claude, we are promoting Emergence from a macro-population tool to a **living, breathing 190/100 simulation ecosystem**. We are not adding expensive AI loops; we are using **Data-Oriented Math, Cellular Automata, and Vector Physics**. You will execute the exact architecture detailed below. No compromises on performance.

---

## Phase 1: The Flora Cellular Automata (Ecology)
**File Targets:** `crates/emergence-core/src/world/resource.rs`
Currently, `ResourceLayer` treats everything as static capacities. We are converting `Flora` to an autonomous cellular array.

1. **New Data Structures:** Add parallel flat arrays to `ResourceLayer`:
   - `pub flora_age: Vec<u8>` (0 = None, 1 = Sapling, 2 = Adult, 3 = Elder)
   - `pub flora_hydration: Vec<u8>`
   - `pub flora_energy: Vec<u16>`

2. **The Rules Engine (`tick_flora` system):** Create an optimized loop that executes periodically (e.g., every 60 or 120 ticks) against the arrays:
   - **Growth:** `energy[idx] = energy[idx].saturating_add(hydration[idx] / 10 + 1)`. If `energy > threshold`, `age` increments and `energy` resets.
   - **Reproduction & Seeds:** If `age[idx] == Adult` (2) or `Elder` (3), perform a highly-optimized deterministic Hash check:
     `if (cell_hash(x, y) ^ world_tick) % 100 < 5 { ... }` 
     If the 5% chance hits, check the 8 neighboring cells. If an adjacent cell is empty and its terrain is `Grassland/Forest`, spawn a Sapling (`age=1`, `energy=0`) in that target cell.

---

## Phase 2: Autonomous Fauna (Boids & Finite State Physics)
**File Targets:** Create `crates/emergence-core/src/simulation/fauna.rs`
Beings handle socio-memetic civilization. We need a secondary ECS specifically for wildlife (Deer, Wolves, Fish) that uses physics/flocking logic instead of complex pathfinding.

1. **The Data Structure:**
   - Define a pure data contiguous struct `pub struct Animal { pub pos: [f32; 2], pub vel: [f32; 2], pub species: Species, pub hunger: u8, pub fear: u8, pub age: u8 }`.
   - Maintain a `Vec<Animal>` array inside a new `FaunaSystem`.

2. **The Physics Engine (`update_boids` system):**
   - **Needs Decay:** Every tick, `hunger` increases.
   - **Desire Vectors:** Compute forces. 
     `Velocity = (Flee * 3.0) + (Seek_Food * 1.5) + (Wander * 0.5)`
     - A Deer’s `Seek_Food` vector points to the nearest world cell where `ResourceLayer.flora_age > 1` & `Biome == Grassland`.
     - A Wolf's `Seek_Food` vector points to the nearest Deer in the `Vec<Animal>`.
   - **Lifecycle Breeding:** When a Deer’s `hunger` is near zero, its `mating` drive rises. If two mature animals of the same species physically overlap with high mating drives, instantiate a new juvenile `Animal` at their coordinate.

---

## Phase 3: Dynamic Weather Fields (Climate)
**File Targets:** `crates/emergence-core/src/world/climate.rs`
Weather must be a mathematical consequence, not just a visual filter.

1. **The Field Calculation:**
   - Define a drifting Simplex Noise layer: `rain_density(x, y) = simplex(x * scale + wind_dx * tick, y * scale + wind_dy * tick)`.
2. **Environmental Ramifications:**
   - When a cell’s `rain_density > 0.6`, the weather is "Raining".
   - Raining forces the underlying `ResourceLayer.flora_hydration[idx]` to increase by `+10`.
   - Unclothed Beings standing in a Raining cell receive a deterministic penalty to their `comfort` memetic field, which naturally influences them to retreat toward the closest `Structure` signal.

---

## Phase 4: Deep Crust Resources (Underground Economy)
**File Targets:** `crates/emergence-core/src/world/terrain.rs` & `terrain_gen.rs`
Provide massive economic depth via static topological yields.

1. **Generation:** In WorldGen, lay down hidden noise arrays: `pub oil_deposits: Vec<u16>`, `pub iron_veins: Vec<u16>`.
2. **Extraction Engine:** These finite values lie dormant eternally. Only when the player or the Beings instantiate a specific `StructureType::Extrapump` on that exact cell, a structure-tick fires: 
   - `if terrain.oil_deposits[idx] > 0 { terrain.oil_deposits[idx] -= 1; global_kingdom_oil += 1; }`
   - Once it hits 0, visually transition the Extrapump to a `dry/broken` sprite state.

---

**Claude, begin by laying out the Data Structures (Phase 1 and Phase 4) in the core layer.** Provide the struct definitions, initialization code for `WorldGen`, and outline the mathematical looping logic so we can begin grafting this into the CPU without dropping FPS. Let's build a masterpiece.
