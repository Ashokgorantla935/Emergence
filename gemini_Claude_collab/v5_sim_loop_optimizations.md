# Swarm OS - CPU Simulation Bottleneck Resolutions (v5)

Hey Claude! It's Antigravity. I've heard the GPU shaders are now fully resolved and rendering perfectly on macOS (no more TDRs), but the CPU sim loop is crushing the 22ms tick budget. 

The user was right: `resource.rs` running `O(4M)` scalar passes every tick and `settlement.rs` dynamically allocating `33.5 MB` of heap arrays every 50 ticks is destroying our framerate when time accelerates. 

Here is my target architectural plan. Please sweep these specific optimizations into the rust engine:

## 1. settlement.rs: Rethink the Algorithm (Bypass the 32MB Buffer completely)

Presently, `detect_settlements` loops across the entire 4-million grid mapping and allocates two `w * h` vectors (`cell_label` and `parent` representing 33.5 MB in `Vec<i32>`).

**Do NOT pre-allocate buffers for this.** We have a much better architectural plan. Settlements inherently form *around people*. Instead of searching 4 million inactive tiles, we should invert the search and only look at grid cells occupied by active `Human` entities.

**Action to Take in `detect_settlements`:**
1. Rip out the `vec![-1i32; w * h]` buffer allocations. 
2. Instead of iterating `for y in 0..h { for x in 0..w { ... } }`, iterate specifically over active beings:
   ```rust
   let mut candidate_cells: std::collections::HashSet<usize> = std::collections::HashSet::new();
   for i in 0..beings.hot.count {
       if beings.hot.states[i] == crate::being::data::BeingState::Dead { continue; }
       if beings.hot.creature_type[i] != crate::being::data::CreatureType::Human as u8 { continue; }

       let pos = beings.hot.positions[i];
       let x = pos[0].max(0.0) as u32;
       let y = pos[1].max(0.0) as u32;
       let idx = (y * w as u32 + x) as usize;

       // Only check comfort and count_in_radius on cells that actually contain a human.
       let comfort = signals.read(SignalChannel::Comfort, x, y);
       if comfort >= 0.15 {
           if spatial.count_in_radius(pos[0], pos[1], 4.0) >= 2 {
               candidate_cells.insert(idx);
           }
       }
   }
   ```
3. Execute the Union-Find/Merge clustering *solely* on `candidate_cells` (which contains ~10-100 elements) via a `HashMap` rather than tracking 4 million empty slots.
This changes `detect_settlements` from $O(\text{MapSize})$ (16 ms + 33MB heap thrashing) to $O(\text{Population})$ (~0.1ms execution with practically zero memory layout).

## 2. resource.rs: The CPU Throttle (Execute Every 20 Ticks)

The user perfectly identified the other loop: running multiple $O(\text{MapSize})$ terrain checks every tick across 4 million cells takes anywhere from 5-15ms depending on the compiler layout.

**Action to Take in `tick.rs` & `resource.rs`:**
1. Pass the global `world.tick` clock into `resource.tick_with_laws(..., tick: u32)`. 
2. Inside `resource.rs`'s update method, insert an early return throttle:
   ```rust
   if tick % 20 != 0 { return; }
   ```
3. Inside the `!no_food_regrowth` condition, simply multiply the regrowth payload by `20.0` so we mathematically compensate for the delay without executing 20 separate mathematical loops.
   ```rust
   self.food[i] += self.regrowth_rate[i] * season_multiplier * 20.0;
   ```
4. For the exponential summer decay penalty, apply logarithmic staggering. Replace `0.998` with `0.960` (which is $0.998^{20}$). 

## 3. Pixel Art Integration
Also, as a callback to the user's note: ensure that whatever you compile correctly leverages the pixel art assets we've stitched. If there's any remaining manual hooking needed for the sprite system to target `combined_npcs.png` flawlessly, assert that it maps correctly! 

Apply these sweeps and we'll sail directly past 60 FPS natively!
