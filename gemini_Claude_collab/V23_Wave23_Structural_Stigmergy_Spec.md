# Emergence Wave 23: Structural Stigmergy & Visual Alignment

## Objective
The current building generator correctly parses AI assets but maps coordinate frames to `1/8` by `1/7` splits. Recently generated 1024x1024 assets map exactly to `4x4` grids. Mismatches duplicate 4 miniature "tents" over checkerboard voids on the game screen.
Additionally, Beings suffer from "Petri-Dish Syndrome". Nomads randomly wander out into the void immediately after building campfires because structure locations do not emit gravity/stigmergy onto the environmental calculation map.

We resolve both the architecture sprites and implement chemical stigmergic bounding.

---

## 1. Visual Object Sprite Alignment

### File: `crates/emergence-viewer/src/renderer/objects.rs`
1. Re-map the `BUILD_CELL_` constants from the legacy 8x7 atlas mapping to the standard 4x4 DALL-E grid format.
```rust
// Building spritesheet layout (4 cols × 4 rows)
const BUILD_CELL_U: f32 = 1.0 / 4.0;
const BUILD_CELL_V: f32 = 1.0 / 4.0;
```

### File: `crates/emergence-viewer/src/renderer/shaders/object_sprite.wgsl`
1. Inject a greyscale checkerboard mask. DALL-E generates true `#cccccc` to `#ffffff` Photoshop-style checkerboard grids.
```wgsl
    // Immediately after textureSample:
    let r = color.r;
    let g = color.g;
    let b = color.b;

    // Aggressive greyscale checking to discard DALL-E checkerboard
    let is_grey = abs(r - g) < 0.08 && abs(g - b) < 0.08;
    if (is_grey && r > 0.45) { // Any bright grey/white is checkerboard background
        discard;
    }
```

---

## 2. Infrastructure Signal Emitters (Stigmergy)

The `SignalGrid` propagates simulated chemistry across the grid. Structures must emit `Comfort` to pull their citizens back home.

### File: `crates/emergence-core/src/sim/tick.rs`
1. In the "Signal tick" section (`Step 3`), iterate over the physical structure array and pulse signals directly into the environment. 
2. Add this block **BEFORE** the `reaction_step()` completes.
```rust
    // 3a. Infrastructure Stigmergy (Pulse signals from structures)
    {
        let w = world.terrain.width;
        let h = world.terrain.height;
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let struct_val = world.terrain.structure[idx];
                if struct_val > 0 {
                    // Inject robust Comfort to keep beings orbiting their camps
                    if struct_val == crate::world::terrain::StructureType::Campfire as u8 {
                        world.signals.deposit(SignalChannel::Comfort, x, y, 0.1);
                        world.signals.deposit(SignalChannel::Danger, x, y, -0.05); // Depress danger locally
                    } else if struct_val == crate::world::terrain::StructureType::LeanTo as u8 {
                        world.signals.deposit(SignalChannel::Comfort, x, y, 0.15);
                    } else if struct_val == crate::world::terrain::StructureType::Hut as u8 {
                        world.signals.deposit(SignalChannel::Comfort, x, y, 0.25);
                    }
                }
            }
        }
    }
```

---

## 3. Homelessness Prevention & Hearth Gravity

When a human `Wanders` or evaluates `Cluster`/`SeekShelter`, they must succumb to the gravity of the `Comfort` chemical gradient the camp drops. 

### File: `crates/emergence-core/src/being/actions.rs`
1. Modify `Action::SeekShelter` mapping inside `score_actions` to prioritize navigating along the peak of the Comfort signal.
2. If `Comfort` gradient leads somewhere naturally, drift humans along it rather than picking raw distances.
```rust
            Action::SeekShelter => {
                let [gx, gy] = local.gradients[CH_COMFORT];
                let shelter_pos = find_nearest_shelter(pos, radius, terrain);
                if gx.abs() > 0.03 || gy.abs() > 0.03 {
                    // Drift along comfort gradient to find "home" before relying on raw struct proximity
                    let mut t = [pos[0] + gx * 8.0, pos[1] + gy * 8.0];
                    t[0] += (rng.f32() - 0.5) * 1.5;
                    t[1] += (rng.f32() - 0.5) * 1.5;
                    target_pos = Some(t);
                    score *= 1.2;
                } else if let Some(mut t) = shelter_pos {
                    // Jitter so they cluster AROUND the shelter organically
                    t[0] += (rng.f32() - 0.5) * 1.5;
                    t[1] += (rng.f32() - 0.5) * 1.5;
                    target_pos = Some(t);
                } else {
                    score *= 0.1; // no shelter nearby, penalize
                }
            }
```

3. Do the same logic to augment `Wander`. Suppress purely random movements if `Comfort` gradient is detected, softly coercing humans to "wander tightly near the campfire". This single tweak produces immense behavioral emergence.

## 4. Coastal Sliding Obstacle Avoidance

Beings currently jitter and clump in concave corners when their `Wander` target inadvertently places them in the ocean due to the Scent gradient pushing them outward. 

### File: `crates/emergence-core/src/sim/movement.rs`
1. Inside the `move_toward` function, replace the `else` block for bounding handling (`// Land being hit water boundary — BOUNCE to prevent infinite sticking`) with a Smart Slide & Jitter mechanism.
2. The logic should:
   - Check if the entity can move along the X-axis alone.
   - Check if the entity can move along the Y-axis alone.
   - If trapped in a concave corner where neither axis is valid, generate a deterministic pseudo-random jitter using `world.tick` and `being_index` to forcefully bounce them away.
```rust
    } else {
        // Path blocked by water obstacle. Implement "smart sliding"
        world.beings.hot.velocities[being_index] = [0.0, 0.0];
        
        let try_x = (pos[0] + nx * clamped_dist).clamp(0.0, world.terrain.width as f32 - 1.0);
        let try_y = (pos[1] + ny * clamped_dist).clamp(0.0, world.terrain.height as f32 - 1.0);
        
        let cx = pos[0] as u32;
        let cy = pos[1] as u32;
        let can_x = !world.terrain.is_water_f(try_x, pos[1]);
        let can_y = !world.terrain.is_water_f(pos[0], try_y);
        
        if can_x && !can_y {
            world.beings.hot.positions[being_index][0] = try_x;
            world.beings.hot.velocities[being_index][0] = (nx * clamped_dist).clamp(-MAX_VEL, MAX_VEL);
        } else if can_y && !can_x {
            world.beings.hot.positions[being_index][1] = try_y;
            world.beings.hot.velocities[being_index][1] = (ny * clamped_dist).clamp(-MAX_VEL, MAX_VEL);
        } else {
            // Completely trapped in concave corner. Bounce away!
            let jitter_x = ((world.tick.wrapping_mul(being_index as u64) % 100) as f32 / 50.0) - 1.0;
            let jitter_y = (((world.tick + 1).wrapping_mul(being_index as u64) % 100) as f32 / 50.0) - 1.0;
            let esc_x = (pos[0] + jitter_x * 0.5).clamp(0.0, world.terrain.width as f32 - 1.0);
            let esc_y = (pos[1] + jitter_y * 0.5).clamp(0.0, world.terrain.height as f32 - 1.0);
            if !world.terrain.is_water_f(esc_x, esc_y) {
                world.beings.hot.positions[being_index] = [esc_x, esc_y];
            }
        }
    }
```

---

## 5. Architectural Clearance & Anti-Spam

Currently, humans build campfires indiscriminately, overlaying them onto trees and spamming them next to each other.

### File: `crates/emergence-core/src/being/actions.rs`
1. Inside `score_actions` where `Action::Build` is evaluated, inject two constraints:
   - Zero the score entirely if `terrain.structure[cell_idx] != 0` or if the tile is water.
   - Introduce a steep anti-spam penalty: If `find_nearest_shelter(pos, 15.0, terrain).is_some()`, multiply the score by `0.1` to prevent stacking campfires in the same area.

### File: `crates/emergence-core/src/sim/movement.rs`
1. When `Action::Build` finishes progress (where `terrain.place_structure` is invoked), force the `ResourceLayer` to physically clear the flora on that tile.
2. Inject:
```rust
    // Clear flora footprint (chop trees down to make space)
    world.resources.flora_stage[cidx] = 0;
    world.resources.flora_energy[cidx] = 0;
    world.resources.flora_hydration[cidx] = 0;
```

---

## Conclusion & Verification
Execute the changes across `viewer` and `core`, ensure `cargo build --lib` passes without borrowing issues, and hand the reigns back to the God simulator to evaluate their petri-dish civilizations maturing into actual anchored camps!
