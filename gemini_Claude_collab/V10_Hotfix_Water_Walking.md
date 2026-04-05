# V10 Hotfix Protocol: The Water Walking Glitch

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** High Priority UI/Physics Bug

**Issue Analysis:**
The User reported that beings are spawning in the ocean and appearing to "run" out into deep water. My architectural review reveals two separate root causes for this visual bug:

1. **Spawning in Water `scenario.rs`:** In `SpawnMode::TwoClusters`, the initial centers (`left` and `right`) are hardcoded to `[w * 0.25, h * 0.5]` and `[w * 0.75, h * 0.5]`. If the random terrain seed generates an ocean at those coordinates, `jitter_from` falls back to the exact center when it fails its 20 attempts, effectively dropping the tribes into the Mariana Trench.
2. **Velocity Ghosting `movement.rs`:** In `sim/movement.rs::move_toward`, we properly block non-fish beings from updating their `positions` if the step lands on water (`if !is_water`). *However*, if that check fails, we simply skip the block. We don't zero out the `velocities`. Therefore, the being's velocity vector gets "stuck" at whatever speed it was moving when it hit the coast, causing the graphical renderer to interpolate them running endlessly into the sea.

---

### Execution Instructions:

#### 1. Fix `sim/movement.rs`
Locate the `move_toward` function. Underneath the `is_fish` check, you must explicitly handle the failure conditions by zeroing the velocity:
```rust
    // Fish move in water; all others avoid water
    if is_fish {
        if is_water {
            // ... (keep existing water move logic)
        } else {
            // Fish hit land
            world.beings.hot.velocities[being_index] = [0.0, 0.0];
        }
    } else {
        if !is_water {
            // ... (keep existing land move logic)
        } else {
            // Land being hit water
            world.beings.hot.velocities[being_index] = [0.0, 0.0];
        }
    }
```

#### 2. Fix `scenario.rs`
In `create_world_from_scenario`, when building `SpawnMode::TwoClusters`, do not blindly use the hardcoded `left` and `right` arrays. Instead, locate the points in the `walkable` array that are closest to `[w * 0.25, h * 0.5]` and `[w * 0.75, h * 0.5]`. Use those closest walkable coordinates as the `center` for `jitter_from`.
*(And if `walkable` is empty, just fallback to whatever, but it shouldn't be empty).*

---
**Claude**, apply these fixes to stabilize Wave 10. Once confirmed, we will advance to the Wave 11 Parity Roadmap.
