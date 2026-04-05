# V28: 100-Year Civilization Scale (Time Dilation Math)

## Overview
Currently, the simulation operates on a "petri-dish" time scale designed for rapid generation testing. In this scale, 1 year is defined mathematically as **28,800 ticks**. However, the human lifespan is generated as 3 to 5 "years", causing them to die of old age after 4-5 minutes of real-world gameplay at 1x speed. 

We are transitioning the engine to a realistic **100-Year Human Lifespan**. Because beings will live 20x longer, they will have exponentially more time to build, breed, and consume. We must simultaneously scale reproduction cooldowns and structure structural decay so the game does not explode with overpopulation or require rebuilding houses every 2 months.

## Execution Plan

### 1. Re-Scaling Lifespans (The 100-Year Math)
A "Year" is `28,800` ticks. 100 Years is `2,880,000` ticks.

**File:** `crates/emergence-core/src/scenario.rs`
1. Scale the human `lifespan` variable generated during spawn (`create_world_from_scenario`):
   - **Current:** `let lifespan = 86400 + rng.u32(0..57601);` // 3-5 years
   - **Fix:** `let lifespan = 2_304_000 + rng.u32(0..576_001);` // 80 to 100 years

**File:** `crates/emergence-core/src/lib.rs`
1. In `spawn_fauna`, scale the animal `lifespan` generated during spawn:
   - **Current:** `let lifespan = 86400 + rng.u32(0..57601);`
   - **Fix:** `let lifespan = 432_000 + rng.u32(0..144_001);` // 15 to 20 years

### 2. Fixing the "Elder" Trait Hardcode
**File:** `crates/emergence-core/src/being/lifecycle.rs`
The `check_and_award_traits` function incorrectly hardcodes Elder status to 2.5 years (`72,000` ticks). This must be made dynamic to support the new lifespan scale.

1. **Fix:** Change the `age > 72000` check to calculate Elder status using the same ratio as `life_phase()`:
```rust
let lifespan = beings.hot.lifespans[idx];
let elder_threshold = (lifespan as f32 * 0.85) as u32;

if age > elder_threshold {
    *traits |= BEING_TRAIT_ELDER;
}
```

### 3. Preventing the Baby Explosion (Breeding Cooldowns)
**File:** `crates/emergence-core/src/sim/tick.rs`
Humans living 100 years with a 400-tick reproduction cooldown will cause a complete mathematical breakdown of the simulation via overpopulation.

1. In the `// Fauna breeding check` section, locate the breeding cooldown assignments:
   - **Current:** `world.beings.hot.breeding_cooldowns[p_a] = 400;`
   - **Fix:** Update the cooldown to a massive `14400` ticks (roughly 6 in-game months) for humans, and `2400` ticks (~1 month) for fauna.
   - *Note:* Ensure you differentiate between humans and fauna here if possible. If they use the same generic block, you can check `creature_type`.

### 4. Making Civilization Architecture Permanent
**File:** `crates/emergence-core/src/world/terrain.rs`
Buildings currently decay and magically disappear into dust after exactly `5,000` ticks (less than 2 in-game months!). We need civilization to endure.

1. In `decay_structures()` modify the arbitrary `self.structure_age[idx] >= 5000` check. Replace it with a dynamic lifespan based on `StructureType`:
   - Basic Shelters / Dirt Paths (`Campfire`, `LeanTo`, `Hut`, `NomadTent`, `ResourceCache`): `144_000` ticks (5 years)
   - Intermediate Wood (`WoodenHouse`, `Windmill`, `Wall`, `Automobile`): `576_000` ticks (20 years)
   - Advanced Stone (`StoneHouse`, `Keep`, `Castle`, `Factory`, `Mine`, `Forge`): `2_880_000` ticks (100 years)
   
*Note: Ensure to keep `tick.rs` structure decay synchronized if it manually advances `structure_age` by 100 (which it does in `tick.rs` line 954).*

---
**God Architect's Note to Claude:** This brings our civilization into a realistic Epochal scale. Execute these numerical updates with pristine precision to avoid destroying the underlying balance of hunger, warmth, and lifespan constraints.
