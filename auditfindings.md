# Performance Audit Findings — 2026-04-02

## Executive Summary
The Emergence god game runs at 5-20 FPS on 2048x2048 maps. The bottleneck is **NOT** GPU readback or signal diffusion. It is **O(WorldSize) CPU loops** executing every tick on 4,194,304 cells.

## Root Causes

### 1. Resource Tick — EVERY TICK, ~6-10ms
**File:** `crates/emergence-core/src/world/resource.rs:110-175`

`tick_with_laws()` runs every tick:
- Line 121: `for i in 0..self.food.len()` — iterates ALL 4M cells for food regrowth
- Line 136 (Summer): Full 2048x2048 nested loop for drought zone detection
- Line 158 (Spring): Full 2048x2048 nested loop for flood plain food boost

Each touches 4M cells. In Summer/Spring: **8M+ cell reads/writes per tick**.

### 2. Settlement Detection — Every 50 Ticks, ~20-50ms spike
**File:** `crates/emergence-core/src/sim/settlement.rs:63-143`

Every 50 ticks:
- Allocates `Vec<i32>` of 4M elements (16MB) for cell_label
- Allocates another `Vec<i32>` of 4M elements (16MB) for union-find parent
- 3 full world scans (comfort check, union-find merge, grouping)
- `count_in_radius()` spatial queries for every cell with comfort >= 0.15

**32MB heap allocation + 12M+ cell visits every 50 ticks.**

### 3. Periodic Full-World Scans
- `tick.rs:82-114` — Sea level rise: 4M cells every 200 ticks
- `tick.rs:534` — Trauma engram: sums all 4M danger values every 100 ticks
- `tick.rs:665` — Food regrowth (subsampled): every 100 ticks, step_by(4)

### 4. Speed Multiplier Interaction
**File:** `crates/emergence-app/src/main.rs:991-1000`

At 5x speed, the engine tries 5 ticks/frame. But one tick alone takes ~20ms (exceeds 8ms budget). Budget check fires after first tick, so at least 1 full tick always runs.

## Frame Budget Breakdown (Steady State)

| Component | Cost | Frequency |
|-----------|------|-----------|
| Resource tick (food + seasons) | ~6-10ms | Every tick |
| Being AI (rayon parallel) | ~5-8ms | Every tick |
| Signal GPU cycle | ~6ms | Every frame |
| Spatial index rebuild | ~1-2ms | Every tick |
| egui rendering | ~1-12ms | Every frame |
| **Total** | **~20-40ms** | **= 25-50 FPS** |

## Spike Events (5 FPS / 200ms frames)

| Event | Cost | Frequency |
|-------|------|-----------|
| Settlement detection | +20-50ms | Every 50 ticks |
| Trauma/danger sum | +5-10ms | Every 100 ticks |
| Structure decay | +3-5ms | Every 100 ticks |

## What's Already Optimized (NOT the problem)
- GPU readback: async via map_async + AtomicBool + Maintain::Poll
- Signal diffusion: on GPU (gpu_managed=true skips CPU entirely)
- Rayon parallel diffusion: added but only active when gpu_managed=false
- Terrain rendering: viewport-bounded with LOD stride
- Object rendering: chunk-based (32x32 grid)
- Being rendering: frustum culling added

## Recommended Fixes

### Quick Wins (no architecture change)
1. **Throttle resource regrowth** to every 10-20 ticks. Food change is imperceptible at 1-tick granularity. Saves ~6-10ms/tick.
2. **Throttle seasonal weather scans** to every 100+ ticks. Drought/flood effects accumulate slowly.
3. **Pre-allocate settlement buffers** on World struct. Reuse instead of 32MB allocation every 50 ticks.
4. **Merge double RwLock write acquisitions** at main.rs lines 960 and 986.

### Medium-Term
5. **Move resource regrowth to GPU compute** — embarrassingly parallel per-cell update.
6. **Downsample settlement detection** — scan every 4th cell, or only cells with nonzero comfort signal.

### Strategic
7. **Consider map size tiers** — 2048x2048 with per-tick full-world CPU scans will never hit 60 FPS without GPU offloading or aggressive throttling.
