# Wave 14: Consolidated Stability & Performance Fix

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**
**Priority: CRITICAL — User reports 4-5 FPS at 1x speed**

## Status Audit

Waves 10-12 were originally planned as feature work (Atlas upgrade, Societal AI, World Gen) but were consumed by emergency bug fixes:
- **Wave 10** → became the 1024x1024 atlas migration (executed)
- **Wave 11** → became async GPU readback fix for `Maintain::Wait` stall (executed)
- **Wave 12** → became terrain viewport caching to stop 4.5M instance rebuilds (executed)
- **Wave 13** → entity rendering decoupling spec (NOT yet executed — beings still glitchy)

After those fixes, the user restored a deleted line and the game currently shows **4-5 FPS** in the TopBar even though it "feels OK." This wave consolidates all remaining stability work into one surgical execution.

---

## Current Architecture (Important Context)

The entire app is **single-threaded**. The `Arc<RwLock<World>>` is NOT contended across threads — simulation ticks and rendering happen sequentially in the same `update()` call on the main thread. The flow is:

```
update() called by winit event loop:
  1. FPS tracking (line 831-837) ← WORKING
  2. Simulation ticks (line 840-1057) ← step_n() runs synchronously
  3. TPS tracking (line 1061-1068) ← WORKING
  4. Camera update (line 1070)
  5. World read for GPU buffer updates (line 1252-1387) ← read lock, no contention
  6. egui UI pass (line 1390-2277)
  7. GPU render pass (line 2299-2468)
  8. Signal compute pass (line 2306-2333) ← async readback, WORKING
  9. Present (line 2555)
```

The `world.write().unwrap()` calls on lines 890, 916, 1170, 2310 are all on the same main thread and never contend. **Lock contention is NOT the FPS issue.**

---

## Root Cause Analysis: Why 4-5 FPS

### Hypothesis 1: The FPS counter is just wrong
The FPS tracking at line 831-837 is correct code. `current_fps` is computed as `frames_since_last_sec` after 1 second elapses. This feeds into line 1439. If the user sees "4-5 FPS" and it "feels OK," the counter IS accurate — the frame rate really is 4-5 FPS.

### Hypothesis 2: Per-frame work is too heavy (~200ms per frame)
At 1x speed, `ticks = 1`, so `step_n` runs 1 tick. Each tick is fast (~2ms). But the **render side** does heavy work every frame:

1. **Campfire particle scan** (line 1298-1312): Iterates ALL terrain cells `tw * th` (up to 2048×2048 = 4.19M cells) scanning for campfire structures. **This runs EVERY 6 ticks** with a nested loop. On a 2048×2048 map, this is a 4M iteration scan.

2. **Object renderer update** (line 1272-1273): `obj.update()` calls `self.rebuild()` **every 4 ticks** (line 544 of objects.rs: `self.frame_tick % 4 == 0`). The rebuild iterates all `w * h` cells twice (resources + decorations), building up to 35K instances and writing ~1.7MB to the GPU.

3. **Kingdom overlay** (line 1384-1387): `build_kingdom_frame()` + `ko.prepare()` runs every frame.

4. **Being renderer** (line 1260-1265): `br.update()` iterates all beings every frame.

### The Fix

The campfire scan and object rebuild are the biggest offenders. They need frequency gating and viewport culling.

---

## Execution Plan

### Fix 1: Gate the campfire particle scan (CRITICAL)

**File: `crates/emergence-app/src/main.rs`, around line 1298**

Current code scans ALL `tw * th` terrain cells every 6 ticks looking for campfires:
```rust
if frame_tick % 6 == 0 {
    let tw = world.terrain.width as usize;
    let th = world.terrain.height as usize;
    for y in 0..th {
        for x in 0..tw {
```

**Replace with viewport-culled scan:**
```rust
if frame_tick % 6 == 0 {
    let tw = world.terrain.width as usize;
    let th = world.terrain.height as usize;
    // Only scan visible viewport for campfire particles
    let half_w = (self.camera.zoom * self.camera.aspect * 0.5 + 4.0) as usize;
    let half_h = (self.camera.zoom * 0.5 + 4.0) as usize;
    let cx = self.camera.position[0] as usize;
    let cy = self.camera.position[1] as usize;
    let x_min = cx.saturating_sub(half_w);
    let x_max = (cx + half_w).min(tw);
    let y_min = cy.saturating_sub(half_h);
    let y_max = (cy + half_h).min(th);
    for y in y_min..y_max {
        for x in x_min..x_max {
```

This changes a 4.19M cell scan into a ~10K cell scan (visible viewport only). Campfire smoke outside the viewport doesn't need to be rendered.

### Fix 2: Reduce object rebuild frequency

**File: `crates/emergence-viewer/src/renderer/objects.rs`, line 544**

Current: rebuilds every 4 ticks for campfire animation flicker.
```rust
let needs_rebuild = self.dirty || ppu_changed || (self.frame_tick % 4 == 0);
```

**Change to every 30 ticks** (campfire flicker is still smooth at 0.5s intervals):
```rust
let needs_rebuild = self.dirty || ppu_changed || (self.frame_tick % 30 == 0);
```

Also, the `ObjectRenderer` is wrapped in `if false { ... }` at line 2403-2419, meaning it's **completely disabled** in rendering. But `obj.update()` at line 1272 STILL runs every frame, rebuilding 35K instances that are never drawn.

**Either:**
- (a) Also wrap the `obj.update()` call in the same `if false` guard, OR
- (b) Re-enable the object renderer draw call (remove the `if false` wrapper)

I recommend **(a)** — skip the update entirely since the draw is disabled:
```rust
// Object renderer update — DISABLED (draw call is also disabled)
// if let Some(ref mut obj) = self.object_renderer {
//     obj.update(&rs.queue, &world.terrain, &world.resources, pixels_per_unit);
// }
```

### Fix 3: Time-budget the simulation tick loop 

**File: `crates/emergence-app/src/main.rs`, around line 916-917**

Current code runs ALL requested ticks synchronously:
```rust
let mut world = world.write().unwrap();
emergence_core::step_n(&mut world, ticks);
```

At 200x speed, this runs 200 ticks before rendering (~500ms). At 500x, it's ~1.5 seconds.

**Replace with time-budgeted loop:**
```rust
let mut world = world.write().unwrap();
// Time-budgeted ticking: never spend more than 12ms ticking per frame.
// This guarantees ≥60 FPS even at extreme speed settings.
// Remaining ticks are silently dropped — the sim runs slower
// than the label says, which is correct vs freezing.
const TICK_BUDGET_MS: u128 = 12;
let tick_start = Instant::now();
let mut ticked = 0u32;
for _ in 0..ticks {
    if ticked > 0 && tick_start.elapsed().as_millis() >= TICK_BUDGET_MS {
        break; // Budget exhausted — render what we have
    }
    emergence_core::step(&mut world);
    ticked += 1;
}
```

This prevents the 200x/500x freeze. The speed buttons become "aspirational targets" — exactly how Dwarf Fortress and RimWorld handle it.

### Fix 4: Update the `ticks_since_timer` to use actual ticked count

After the time-budgeted loop, the code at line 1056 does:
```rust
self.ticks_since_timer += ticks;
```

This should use the ACTUAL ticks completed, not the requested count:
```rust
self.ticks_since_timer += ticked; // Use actual count, not requested
```

(This requires `ticked` to be visible at line 1056. You may need to hoist it or store it in `self`.)

---

## What NOT to change

- **Signal compute pipeline** (`compute.rs`) — Wave 11 async readback is working correctly.
- **Terrain renderer** (`terrain.rs`) — Wave 12 viewport caching is working correctly.
- **FPS/TPS calculation** — Already implemented and correct.
- **Atlas generator** (`generator.rs`) — Already connected to 1024x1024.

## Verification

After applying these fixes:
1. Launch the game, start any scenario
2. The TopBar FPS should show **50-60 FPS** at 1x speed (currently 4-5)
3. Set speed to 200x — the UI must NOT freeze
4. Set speed to 500x — the UI must stay responsive (sim just runs slower than 500x actual)

## Summary of Changes

| File | Line(s) | Change |
|---|---|---|
| `main.rs` | ~1272-1274 | Disable `obj.update()` (draw is already disabled) |
| `main.rs` | ~1298-1312 | Viewport-cull campfire particle scan |
| `main.rs` | ~916-917 | Time-budgeted tick loop (12ms cap) |
| `main.rs` | ~1056 | Use actual ticked count for TPS |
| `objects.rs` | ~544 | Reduce rebuild frequency to every 30 ticks |
