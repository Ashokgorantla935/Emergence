# V55: The Axioms of Emergence - Fundamental Forces and Energy

## Author: God Architect (Antigravity/Gemini)
## Target: Staff Engineer (Claude)

Claude, V54 safely decoupled the graphics pipeline from the simulation bottleneck. Now, in V55, we architect the core "Binding Layer" of the simulation. 

We are destroying all traditional "If/Then" game logic arrays. The world will be governed entirely by Four Fundamental Grids, Biological Scaling equations, and the Law of Energy Conservation. You are forbidden from hardcoding $O(N^2)$ neighbor sight-checks or manual structure upgrades.

Execute this structural rewrite meticulously.

---

## 1. The Four Fundamental Sensory Grids (Stigmergy)
Beings do NOT loop through entity arrays checking "who is near me" — this causes $O(N^2)$ death. The environment is composed of four continuous floating-point matrices layered directly over the terrain. Beings only query the tile they stand on, following analytical gradients.

1. **Thermodynamic Field (T-Field):** Tracks heat, fire, and shade.
   - *Rule:* High-mass entities (trees, castles) project negative thermal gradients into the matrix equivalent to shade.
2. **Biomass Field (B-Field):** Tracks fertility and food scent.
   - *Rule:* Dropped food injects positive signals. Diffusion causes this signal to blur outward. The AI "smells" the gradient vector and pathfinds toward the peak.
3. **Memetic Field (M-Field):** Tracks fear, panic, and culture belonging.
   - *Rule:* A wolf howling spikes +1000 Fear into the grid. Nearby humans sample the gradient slope and automatically flee outward.
4. **Kinetic Field (K-Field):** Elevation, solid walls, traversal costs.

*Implementation Constraint:* You MUST offload the diffusion (blur/spread) of these signal matrices strictly into your WGPU Compute Shaders (`compute.rs`), returning the processed fields back to the CPU purely for navigation.

---

## 2. Conservation of Energy (The Prime Axiom)
The simulation is a STRICT closed-loop thermodynamic system. Spawning entities with infinite timers will cause mathematically proven memory crashes.

1. Define a global `WORLD_ENERGY_CAP: u64` locked at genesis.
2. Grass, Humans, Animals, and Buildings possess Caloric/Structural Mass drawn directly from this finite pool.
3. Once humans lock all ambient energy into enormous megacities, trees and food physically stop growing. 
4. Famine triggers. This mathematically forces "War" without any hard-coded aggression mechanics. The only way to survive is to destroy neighboring buildings to release locked energy back into the Biomass Field.

---

## 3. Procedural Asset Mapping (Huts to Skyscrapers)
Entities never "choose" to build a castle. They continually execute `Action::Build`. The visual outcome is determined entirely by the Settlement's localized **T-Mass (Technological Wealth)**.

On the `architecture_spritesheet` (8x8):
- **Rows:** Biome / Race variants.
- **Columns (0 to 7):** Tech Tier (Column 0 = Mud Hut, Column 7 = Skyscraper).

**The Mathematical UV Target (Rust -> WGPU):**
```rust
let normalized_tech = (settlement.wealth / MAX_WEALTH_CONSTANT).clamp(0.0, 1.0);
let target_column = (normalized_tech * 7.0).floor() as u32;
// Ensure instance struct maps to `target_column` dynamically
```

---

## 4. The Organic Mass-to-Scale Equation
Remove all hardcoded `scale = 2.0` flags. An object’s rendering area on the GPU must be derived procedurally from its simulated Biological Mass ($M$).
Because it is rendered top-down (Area), visual radius equals the square root of Mass.

**The GPU Scale Injector:**
```rust
let class_visual_constant = 0.1; // Baseline pixel scaling mapping
instance.scale_multiplier = class_visual_constant * f32::sqrt(entity.mass as f32);
```
*Result:* A rabbit born with $Mass = 9$ draws at `0.3x` scale. If a rabbit eats infinite radioactive grass and achieves $Mass = 10,000$, the engine seamlessly renders a gigantic `10.0x` Godzilla-scale rabbit.

---

## 5. Tick Staggering (Utility Brain vs. Fast Kinetics)
The simulation will starve if you process AI goals at 60 ticks per second.
- **The Cognitive Loop:** The Utility AI (Hunger vs. Fear vs. Build) calculates *only* once every $1.0 - 2.0$ seconds. It decides on a "Path Coordinate" and goes to sleep.
- **The Kinetic Loop:** The ultra-fast loop merely pushes velocity arrays toward the memory-cached Coordinate.

Claude, rewrite the foundational `tick()` architecture, integrate the square root Mass-Scaling, and collapse the entity sensory arrays into the Four Grids. Acknowledge when the $O(N^2)$ bottlenecks are dead.
