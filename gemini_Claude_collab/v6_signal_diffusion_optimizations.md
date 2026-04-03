# Swarm OS - CPU Signal Diffusion Optimizations (v6)

Hey Claude! It's Antigravity. Astonishing results from the wave 5 optimizations—we took 16ms of dead weight out of the loop and vaporized the 32MB settlement allocations. We're at 30 FPS!

The final major roadblock is Signal Diffusion taking 5.8ms to 7.1ms per frame (because it diffuses all cells every 2 ticks). If we resolve this, we shatter the 60 FPS ceiling. 

Here are the precise architectural modifications you need to run to optimize `signal.rs`:

## 1. Eliminate the Tight-Loop Iterator Allocations in `diffusion_step`

Right now, in `crates/emergence-core/src/world/signal.rs`, the inner loop iterates $O(4M)$ cells doing this:
```rust
let left = if x > 0 { Some(...) } else { None };
// ... right, up, down ...
let neighbor_count = [left, right, up, down].iter().filter(|n| n.is_some()).count() as f32;
// ...
for nb in [left, right, up, down].into_iter().flatten() {
```
Creating `Option` bindings, wrapping them in arrays, and calling `.iter().filter().flatten()` natively inside the tightest inner loop on a 4-million scalar grid is generating millions of branch computations and destroying the CPU instruction pipeline.

**Action to Take:** 
Rip out the iterator chains and replace it with a direct hardcoded index array.
```rust
let mut neighbors = 0;
let mut n_idx = [0usize; 4];

if x > 0 { n_idx[neighbors] = idx - 1; neighbors += 1; }
else if wrap { n_idx[neighbors] = idx + (w - 1) as usize; neighbors += 1; }

if x + 1 < w { n_idx[neighbors] = idx + 1; neighbors += 1; }
else if wrap { n_idx[neighbors] = idx - (w - 1) as usize; neighbors += 1; }

if y > 0 { n_idx[neighbors] = idx - w as usize; neighbors += 1; }
if y + 1 < h { n_idx[neighbors] = idx + w as usize; neighbors += 1; }

if neighbors > 0 {
    let per_neighbor = bleed / (neighbors as f32);
    for i in 0..neighbors {
        scratch[n_idx[i]] += per_neighbor;
    }
}
```
*Note: If you feel motivated, you can separate the 2D sweep into an interior loop (`1..h-1`, `1..w-1`) which requires exactly ZERO bounds checking, and process edges manually, but even the static array swap above will triple the speed.*

## 2. Stagger Diffusion by Channel (Temporal Amortization)

In `tick.rs`, the current loop calls `diffusion_step()` every 2 ticks, which recursively `par_iter`'s through all active channels, creating a localized spike every other frame. We can chop this down further.

**Action to Take:**
1. Split out `world.signals.tick()` so it takes `tick_count: u32`.
2. Instead of diffusing all $~8$ channels simultaneously every 2 ticks, change the signal engine to diffuse exactly **one channel per tick** (`tick_count % 8`).
3. Multiplying the decay or transmission is *not* necessary; simply delaying the channel diffusions to round robin every 8 ticks visually maintains the exact same macro-level heatmapping behavior while mathematically shattering the 5.8ms workload chunk down to roughly `0.5ms` consistently deployed per frame!

## 3. Warning on Spritesheet Fallback
The user alerted us that if the pixel-art generator breaks and `combined_npcs.png` gets corrupted or goes missing, the system silently falls back to the terrain atlas without telling anyone.
**Action to Take:**
Inject a `log::warn!("NPC Spritesheet combined_npcs.png failed to load - falling back to terrain atlas!");` in the Viewer renderer specifically around wherever the fallback is captured so we can debug asset failures safely.

Apply these, and we'll secure the flawless 60 FPS sim architecture!
