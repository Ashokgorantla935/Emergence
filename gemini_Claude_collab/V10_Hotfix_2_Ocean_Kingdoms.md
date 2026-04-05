# V10 Hotfix Protocol 2: Ocean Kingdoms & Flee Vectors

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Critical Gameplay Logic Bugs

**Issue Analysis:**
The user's footage revealed missing constraints in our God Power implementation. Specifically, the spawn logic places predators too close to tribes, instantly maxing out Danger. The new Flee override bypasses terrain constraints, blasting units straight into the deep ocean. Once in the ocean, `flee_ticks` hits 0, the human calms down, and begins placing Campfires and Huts in the Mariana Trench, accidentally founding Ocean Settlements. 

Here are the precise architectural fixes to execute immediately.

---

### Execution Instructions:

#### 1. Fix the Flee Override Physics (`sim/tick.rs`)
In `tick.rs`, under the `5e-pre2a. Danger flee override`, you have manually implemented `positions[i] = [new_x, new_y]`. This bypasses the structural bounds we just fixed in `movement.rs`.
- Replace the raw position assignment with a terrain check:
```rust
let is_water = world.terrain.is_water(new_x as u32, new_y as u32);
if !is_water {
    world.beings.hot.positions[i] = [new_x, new_y];
} else {
    world.beings.hot.velocities[i] = [0.0, 0.0];
}
```

#### 2. Restrict Building to Land (`sim/movement.rs`)
In `sim/movement.rs`, search for `Action::Build` and `Action::BuildClean`.
- Add a strict condition checking that the central tile is NOT water before allowing construction.
```rust
if world.terrain.structure[cidx] == 0 && !world.terrain.is_water(cx, cy) {
    // Proceed with build target selection and progress
```

#### 3. Throttle the Panic Trigger (`sim/tick.rs`)
Currently, `if danger > 0.85` immediately sets `flee_ticks = 15`. If danger stays high, they remain locked in the Flee state permanently, overriding all other AI.
- Only trigger the 15-tick panic cooldown if they are hit with a *fresh* spike of danger. Modify the initial condition in `tick.rs`:
```rust
if danger > 0.85 && world.beings.hot.flee_ticks[i] == 0 {
    world.beings.hot.flee_ticks[i] = 15;
    // (Drop carry inventory, cancel actions, spike fear)
}
```

---
**Claude**, execute these fixes to stop the creation of Atlantis. Once this physics validation is done, return your full attention to the V11 Sovereignty Spec already assigned.
