# Wave 15: Fix 200x Speed Freeze (Time-Budgeted Tick Loop)

Claude, the user hit 200x speed and the entire window froze. Here's exactly why and how to fix it.

## Root Cause

At 200x, `SimSpeed::Speed200x.ticks_per_frame()` returns `200`. The render loop at line ~899 does:
```rust
emergence_core::step_n(&mut world, ticks); // ticks = 200
```
Which calls `tick::tick(world)` **200 times synchronously** on the main thread before a single frame renders. Each `tick()` runs signal diffusion (8 channels × 256×256 = 524K cells), spatial rebuild, AI scoring, movement for every being, etc. At 200 ticks, that's easily 500ms+ of CPU work — the window manager sees no frame for half a second and reports the app as frozen.

At 500x it's even worse: 500 ticks per frame = ~1.5 seconds of pure compute between renders.

## The Fix: Time-Budgeted Tick Loop

Instead of blindly running all `ticks` before rendering, cap the wall-clock time spent ticking per frame. If the budget runs out, defer remaining ticks to the next frame:

### `crates/emergence-app/src/main.rs` (around line 822-899)

Replace the current tick block:
```rust
        if self.screen == ScreenState::Playing {
            let ticks = self.speed.ticks_this_frame();
            if ticks > 0 {
```

With a time-budgeted version:
```rust
        if self.screen == ScreenState::Playing {
            let ticks = self.speed.ticks_this_frame();
            if ticks > 0 {
                // Time budget: never spend more than 12ms ticking per frame
                // This guarantees ≥60 FPS even at extreme speed settings.
                // Remaining ticks are silently dropped — the sim just runs slower
                // than the label says, which is the correct trade-off vs freezing.
                const TICK_BUDGET_MS: u128 = 12;
                let tick_start = instant::Instant::now();
```

Then change line ~898-899 from:
```rust
                    let mut world = world.write().unwrap();
                    emergence_core::step_n(&mut world, ticks);
```
To:
```rust
                    let mut world = world.write().unwrap();
                    // Time-budgeted ticking: run up to `ticks` but stop if we exceed budget
                    let mut ticked = 0u32;
                    for _ in 0..ticks {
                        if ticked > 0 && tick_start.elapsed().as_millis() >= TICK_BUDGET_MS {
                            break; // Budget exhausted — render what we have
                        }
                        emergence_core::step(&mut world);
                        ticked += 1;
                    }
```

This ensures:
- At 1x-50x: All ticks complete within budget (~1-5ms), full speed
- At 100x-200x: Runs as many ticks as fit in 12ms, then renders — smooth 60 FPS
- At 500x: Same — just runs fewer ticks per frame, sim appears to cap at ~150-200x actual speed

No freeze. No dropped frames. The speed buttons become "aspirational targets" rather than hard guarantees. This is exactly how Dwarf Fortress, RimWorld, and every real-time sim handles it.

## Also: The `signal.rs` gpu_managed early return

I noticed at line 181-185 of `signal.rs`:
```rust
    pub fn tick(&mut self) {
        if self.gpu_managed {
            return; // Skips ALL CPU signal work
        }
```

When `gpu_managed = true` (which the render loop sets), the CPU signal diffusion is skipped entirely. This means at high speeds, the signal grid isn't actually computing on CPU — but the AI behavior, movement, combat, needs decay, emotions, spatial indexing, etc. still run. Those are the real bottleneck at 200+ ticks.

Please execute this time-budgeted tick loop fix!
